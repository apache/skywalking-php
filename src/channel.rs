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

use crate::global::GLOBAL;
use anyhow::{anyhow, bail, Context};
use ipc_channel::ipc::{self, IpcReceiver, IpcSender, TryRecvError};
use once_cell::sync::OnceCell;
use skywalking::reporter::{grpc::ColletcItemConsume, CollectItem, Report};
use std::{
    error::Error,
    sync::{atomic::Ordering, Mutex},
};
use tokio::task;
use tonic::async_trait;
use tracing::{debug, error};

const MAX_COUNT: usize = 1000;

static SENDER: OnceCell<Mutex<IpcSender<CollectItem>>> = OnceCell::new();
static RECEIVER: OnceCell<Mutex<IpcReceiver<CollectItem>>> = OnceCell::new();

pub fn init_channel() -> anyhow::Result<()> {
    let channel = ipc::channel()?;

    if SENDER.set(Mutex::new(channel.0)).is_err() {
        bail!("Channel has initialized");
    }

    if RECEIVER.set(Mutex::new(channel.1)).is_err() {
        bail!("Channel has initialized");
    }

    Ok(())
}

fn channel_send(data: CollectItem) -> anyhow::Result<()> {
    let old_count = GLOBAL.channel_size.fetch_add(1, Ordering::SeqCst);
    if old_count >= MAX_COUNT {
        bail!("Channel is fulled");
    }
    debug!("Channel remainder count: {}", old_count);

    SENDER
        .get()
        .context("Channel haven't initialized")?
        .lock()
        .map_err(|_| anyhow!("Get lock failed"))?
        .send(data)?;

    Ok(())
}

fn channel_receive() -> anyhow::Result<CollectItem> {
    let receiver = RECEIVER
        .get()
        .context("Channel haven't initialized")?
        .lock()
        .map_err(|_| anyhow!("Get lock failed"))?;

    let r = receiver.recv();
    GLOBAL.channel_size.fetch_sub(1, Ordering::SeqCst);
    Ok(r?)
}

fn channel_try_receive() -> anyhow::Result<Option<CollectItem>> {
    let receiver = RECEIVER
        .get()
        .context("Channel haven't initialized")?
        .lock()
        .map_err(|_| anyhow!("Get lock failed"))?;

    let r = match receiver.try_recv() {
        Ok(data) => Ok(Some(data)),
        Err(TryRecvError::Empty) => Ok(None),
        Err(e) => Err(e.into()),
    };
    GLOBAL.channel_size.fetch_sub(1, Ordering::SeqCst);
    r
}

pub struct Reporter;

impl Report for Reporter {
    fn report(&self, item: CollectItem) {
        if let Err(err) = channel_send(item) {
            error!(?err, "channel send failed");
        }
    }
}

pub struct Consumer;

#[async_trait]
impl ColletcItemConsume for Consumer {
    async fn consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        match task::spawn_blocking(channel_receive).await {
            Ok(r) => match r {
                Ok(item) => Ok(Some(item)),
                Err(e) => Err(e.into()),
            },
            Err(e) => Err(Box::new(e)),
        }
    }

    async fn try_consume(&mut self) -> Result<Option<CollectItem>, Box<dyn Error + Send>> {
        match task::spawn_blocking(channel_try_receive).await {
            Ok(r) => match r {
                Ok(item) => Ok(item),
                Err(e) => Err(e.into()),
            },
            Err(e) => Err(Box::new(e)),
        }
    }
}
