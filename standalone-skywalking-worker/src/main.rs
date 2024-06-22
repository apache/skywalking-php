use skywalking_agent::worker::{new_tokio_runtime, start_worker};

fn init_logger() {

}

fn main() -> anyhow::Result<()> {
    let rt = new_tokio_runtime(10);
    rt.block_on(start_worker(""))?;
    Ok(())
}
