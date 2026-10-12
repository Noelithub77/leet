//! GPUI image requests use the same bounded public transport as statement clients.
use std::{path::PathBuf, sync::Arc};
use gpui_kit::{App, BackgroundExecutor, http_client::{AsyncBody, HttpClient, Method, Request, Response, Url, http::HeaderValue}};
use futures::future::BoxFuture;

struct StatementImages { executor: BackgroundExecutor, directory: PathBuf }
pub(crate) fn install(cx: &mut App, directory: PathBuf) {
    cx.set_http_client(Arc::new(StatementImages { executor: cx.background_executor().clone(), directory }));
}
impl HttpClient for StatementImages {
    fn user_agent(&self) -> Option<&HeaderValue> { None }
    fn proxy(&self) -> Option<&Url> { None }
    fn send(&self, request: Request<AsyncBody>) -> BoxFuture<'static, anyhow::Result<Response<AsyncBody>>> {
        let executor = self.executor.clone(); let directory = self.directory.clone();
        Box::pin(async move {
            anyhow::ensure!(request.method() == Method::GET, "Statement images require GET");
            let uri = request.uri().to_string();
            let bytes = executor.spawn(async move { practice::description::media::cached_image(&directory, &uri) }).await?;
            Ok(Response::builder().status(200).body(bytes.into())?)
        })
    }
}
