use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use core::time::Duration;
use crate::interface::RemovePeerError;

pub fn spawn_demo<C, D>(
	cleanup: C,
	duration: Duration,
	token: CancellationToken,
	shutdown_signal: D
) -> JoinHandle<Result<(), RemovePeerError>>
where
	C: core::future::Future<Output = ()> + Send + 'static,
	D: core::future::Future<Output = ()> + Send + 'static
{
	tokio::spawn(async move {
		tokio::select! {
			_ = tokio::time::sleep(duration) => (),
			_ = token.cancelled() => (),
			_ = shutdown_signal => ()
		}

		cleanup.await;

		Ok(())
	})
}
