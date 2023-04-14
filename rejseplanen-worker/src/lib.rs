use worker::*;
use reqwest_wasm::get;

mod utils;

fn log_request(req: &Request) {
    console_log!(
        "{} - [{}], located at: {:?}, within: {}",
        Date::now().to_string(),
        req.path(),
        req.cf().coordinates().unwrap_or_default(),
        req.cf().region().unwrap_or("unknown region".into())
    );
}

#[event(fetch)]
pub async fn main(req: Request, env: Env, _ctx: worker::Context) -> Result<Response> {
    log_request(&req);

    // Optionally, get more helpful error messages written to the console in the case of a panic.
    utils::set_panic_hook();

    // Optionally, use the Router to handle matching endpoints, use ":name" placeholders, or "*name"
    // catch-alls to match on specific patterns. Alternatively, use `Router::with_data(D)` to
    // provide arbitrary data that will be accessible in each route via the `ctx.data()` method.
    let router = Router::new();

    // Add as many routes as your Worker needs! Each route will get a `Request` for handling HTTP
    // functionality and a `RouteContext` which you can use to  and get route parameters and
    // Environment bindings like KV Stores, Durable Objects, Secrets, and Variables.
    router
        .post_async("/", |mut req, _| async move {
            let Ok(form) = req.form_data().await else {
                return Response::error("missing form data", 400)
            };
            let origin = if let Some(FormEntry::Field(value)) = form.get("origin") {
                h3o::CellIndex::try_from(value.as_str().parse::<u64>().unwrap_or_default())
            } else {
                return Response::error("missing origin parameter", 400)
            };

            let destination = if let Some(FormEntry::Field(value)) = form.get("destination") {
                h3o::CellIndex::try_from(value.as_str().parse::<u64>().unwrap_or_default())
            } else {
                return Response::error("missing destination parameter", 400)
            };

            if let (Ok(origin), Ok(destination)) = (origin, destination) {
                let origin_lat_lng = h3o::LatLng::from(origin);
                let destination_lat_lng = h3o::LatLng::from(destination);

                let [olat, olng, dlat, dlng] = 
                    [origin_lat_lng.lat(), origin_lat_lng.lng(), destination_lat_lng.lat(), destination_lat_lng.lng()].map(|x| {
                    (x * 1_000_000.0) as i64
                });

                // make request to rejseplanen api
                let url = format!("http://xmlopen.rejseplanen.dk/bin/rest.exe/trip?originCoordX={}&originCoordY={}&originCoordName=A&destCoordX={}&destCoordY={}&destCoordName=B&format=json", olng, olat, dlng, dlat );
                console_log!("url: {}", url);
                if let Ok(response) = get(url).await {
                    console_log!("response: {:?}", response);
                    if let Ok(text) = response.text().await {
                        return Response::ok(text);
                    }
                }

                Response::error("failed to get response from rejseplanen api", 400)

            } else {
                Response::error("invalid cell index for either origin or destination", 400)
            }
        })

        .get("/worker-version", |_, ctx| {
            let version = ctx.var("WORKERS_RS_VERSION")?.to_string();
            Response::ok(version)
        })
        .run(req, env)
        .await
}
