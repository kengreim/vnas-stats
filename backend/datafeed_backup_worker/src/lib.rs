use js_sys::Date as JsDate;
use worker::{console_error, event, Env, Fetch, Method, Request, Response, Result, ScheduleContext, ScheduledEvent};

/// Health endpoint.
#[event(fetch)]
pub async fn fetch(_req: Request, _env: Env, _ctx: worker::Context) -> Result<Response> {
    Response::ok("datafeed backup worker")
}

/// Scheduled every minute by Cloudflare Cron Trigger.
#[event(scheduled)]
pub async fn scheduled(_event: ScheduledEvent, env: Env, _ctx: ScheduleContext) {
    if let Err(err) = run_backup(&env).await {
        console_error!("backup run failed: {:?}", err);
    }
}

async fn run_backup(env: &Env) -> Result<()> {
    let url = "https://live.env.vnas.vatsim.net/data-feed/controllers.json";
    let bucket = env.bucket("bucket")?;
    let req = Request::new(url, Method::Get)?;
    let mut resp = Fetch::Request(req).send().await?;
    let bytes = resp.bytes().await?;
    let ts_ms = JsDate::now() as u64;
    let object_key = format!("datafeed-{ts_ms}.json");

    bucket.put(object_key, bytes).execute().await?;

    Ok(())
}
