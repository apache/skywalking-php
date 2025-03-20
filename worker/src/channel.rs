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

use skywalking::reporter::{CollectItem, Report};
use std::mem::size_of;
use tokio::{io::AsyncReadExt, sync::mpsc};
use tracing::error;

pub async fn channel_receive(receiver: &mut tokio::net::UnixStream) -> anyhow::Result<CollectItem> {
    let mut size_buf = [0u8; size_of::<usize>()];
    receiver.read_exact(&mut size_buf).await?;
    let size = usize::from_le_bytes(size_buf);

    let mut content = vec![0u8; size];
    receiver.read_exact(&mut content).await?;

    let (item, _) = bincode::serde::decode_from_slice(&content, bincode::config::standard())?;
    Ok(item)
}

pub struct TxReporter(pub mpsc::Sender<CollectItem>);

impl Report for TxReporter {
    fn report(&self, item: CollectItem) {
        if let Err(err) = self.0.try_send(item) {
            error!(?err, "Send collect item failed");
        }
    }
}
