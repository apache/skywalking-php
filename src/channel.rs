// Licensed to the Apache Software Foundation (ASF) under one or more
// contributor license agreements.  See the NOTICE file distributed with
// this work for additional information regarding copyright ownership.
// The ASF licenses this file to You under the Apache License, Version 2.0
// (the "License"); you may not use this file except in compliance with
// the License.  You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use anyhow::{anyhow, bail, Context};
use once_cell::sync::OnceCell;
use skywalking::reporter::{grpc::ColletcItemConsume, CollectItem, Report};
use std::{
    error::Error, io, mem::size_of, os::unix::net::UnixDatagram as StdUnixDatagram, sync::Mutex,
};
use tokio::net::UnixDatagram;
use tonic::async_trait;
use tracing::error;

static SENDER: OnceCell<StdUnixDatagram> = OnceCell::new();
static RECEIVER: OnceCell<Mutex<Option<StdUnixDatagram>>> = OnceCell::new();

pub fn init_channel() -> anyhow::Result<()> {
    let (sender, receiver) = StdUnixDatagram::pair()?;
    sender.set_nonblocking(true)?;
    receiver.set_nonblocking(true)?;

    if SENDER.set(sender).is_err() {
        bail!("Channel has initialized");
    }

    if RECEIVER.set(Mutex::new(Some(receiver))).is_err() {
        bail!("Channel has initialized");
    }

    Ok(())
}

fn channel_send(data: CollectItem) -> anyhow::Result<()> {
    let buf = bincode::serialize(&data)?;

    let sender = SENDER.get().context("Channel haven't initialized")?;

    sender.send(&buf.len().to_le_bytes())?;
    sender.send(&buf)?;

    Ok(())
}

async fn channel_receive(receiver: &UnixDatagram) -> anyhow::Result<CollectItem> {
    let mut size_buf = [0u8; size_of::<usize>()];
    receiver.recv(&mut size_buf).await?;
    let size = usize::from_le_bytes(size_buf);

    let mut buf = vec![0u8; size];
    receiver.recv(&mut buf).await?;

    let item = bincode::deserialize(&buf)?;
    Ok(item)
}

fn channel_try_receive(receiver: &UnixDatagram) -> anyhow::Result<Option<CollectItem>> {
    let mut size_buf = [0u8; size_of::<usize>()];
    if let Err(e) = receiver.try_recv(&mut size_buf) {
        if e.kind() == io::ErrorKind::WouldBlock {
            return Ok(None);
        }
        return Err(e.into());
    }
    let size = usize::from_le_bytes(size_buf);

    let mut buf = vec![0u8; size];
    if let Err(e) = receiver.try_recv(&mut buf) {
        if e.kind() == io::ErrorKind::WouldBlock {
            return Ok(None);
        }
        return Err(e.into());
    }

    let item = bincode::deserialize(&buf)?;
    Ok(item)
}

pub struct Reporter;

impl Report for Reporter {
    fn report(&self, item: CollectItem) {
        if let Err(err) = channel_send(item) {
            error!(?err, "channel send failed");
        }
    }
}

pub struct Consumer(UnixDatagram);

impl Consumer {
    pub fn new() -> anyhow::Result<Self> {
        let receiver = RECEIVER.get().context("Channel haven't initialized")?;
        let receiver = receiver
            .lock()
            .map_err(|_| anyhow!("Get Lock failed"))?
            .take()
            .context("The RECEIVER has been taked")?;
        let receiver =
            UnixDatagram::from_std(receiver).context("try into tokio unix datagram failed")?;
        Ok(Self(receiver))
    }
}

#[async_trait]
impl ColletcItemConsume for Consumer {
    async fn consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        match channel_receive(&self.0).await {
            Ok(item) => Ok(Some(item)),
            Err(e) => Err(e.into()),
        }
    }

    async fn try_consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        match channel_try_receive(&self.0) {
            Ok(item) => Ok(item),
            Err(e) => Err(e.into()),
        }
    }
}
