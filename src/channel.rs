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

use anyhow::anyhow;
use once_cell::sync::OnceCell;
use skywalking::reporter::{CollectItem, Report};
use std::{
    io::Write,
    mem::size_of,
    ops::DerefMut,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    sync::Mutex,
};
use tokio::{io::AsyncReadExt, sync::mpsc};
use tracing::error;

fn channel_send<T>(data: CollectItem, mut sender: T) -> anyhow::Result<()>
where
    T: DerefMut<Target = UnixStream>,
{
    let content = bincode::serialize(&data)?;

    sender.write_all(&content.len().to_le_bytes())?;
    sender.write_all(&content)?;
    sender.flush()?;

    Ok(())
}

pub async fn channel_receive(receiver: &mut tokio::net::UnixStream) -> anyhow::Result<CollectItem> {
    let mut size_buf = [0u8; size_of::<usize>()];
    receiver.read_exact(&mut size_buf).await?;
    let size = usize::from_le_bytes(size_buf);

    let mut content = vec![0u8; size];
    receiver.read_exact(&mut content).await?;

    let item = bincode::deserialize(&content)?;
    Ok(item)
}

pub struct Reporter {
    worker_addr: PathBuf,
    stream: OnceCell<Mutex<UnixStream>>,
}

impl Reporter {
    pub fn new(worker_addr: impl AsRef<Path>) -> Self {
        Self {
            worker_addr: worker_addr.as_ref().to_path_buf(),
            stream: OnceCell::new(),
        }
    }

    fn try_report(&self, item: CollectItem) -> anyhow::Result<()> {
        let stream = self
            .stream
            .get_or_try_init(|| UnixStream::connect(&self.worker_addr).map(Mutex::new))?
            .lock()
            .map_err(|_| anyhow!("Get Lock failed"))?;

        channel_send(item, stream)
    }
}

impl Report for Reporter {
    fn report(&self, item: CollectItem) {
        if let Err(err) = self.try_report(item) {
            error!(?err, "channel send failed");
        }
    }
}

pub struct TxReporter(pub mpsc::Sender<CollectItem>);

impl Report for TxReporter {
    fn report(&self, item: CollectItem) {
        if let Err(err) = self.0.try_send(item) {
            error!(?err, "Send collect item failed");
        }
    }
}
