use kv::kv_server::{Kv, KvServer};
use kv::{GetReply, GetRequest, PutReply, PutRequest};
use std::{collections::HashMap, net::SocketAddr, sync::Arc};
use tokio::{sync::RwLock, task};
use tonic::{Request, Response, Status, transport::Server};
use tracing::{debug, error, info, warn};

pub mod kv {
    tonic::include_proto!("kv");
}

type Store = Arc<RwLock<HashMap<String, Vec<u8>>>>;

#[derive(Clone)]
struct KvSvc {
    store: Store,
}
#[tonic::async_trait]
impl Kv for KvSvc {
    async fn put(&self, req: Request<PutRequest>) -> Result<Response<PutReply>, Status> {
        let kv = req.into_inner();
        debug!("PUT key='{}' value_len={}", kv.key, kv.value.len());
        self.store.write().await.insert(kv.key, kv.value);
        Ok(Response::new(PutReply {}))
    }
    async fn get(&self, req: Request<GetRequest>) -> Result<Response<GetReply>, Status> {
        let key = req.into_inner().key;
        let guard = self.store.read().await;
        let (value, found) = guard
            .get(&key)
            .map(|v| (v.clone(), true))
            .unwrap_or_default();
        debug!(
            "GET key='{}' found={} value_len={}",
            key,
            found,
            value.len()
        );
        Ok(Response::new(GetReply { value, found }))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // initialize tracing
    tracing_subscriber::fmt::init();

    // read CLI flags: our bind addr + comma-separated peer list
    let mut args = std::env::args().skip(1);
    let addr: SocketAddr = args.next().expect("addr").parse()?;
    let peers: Vec<String> = args
        .next()
        .unwrap_or_default()
        .split(',')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    info!("Starting P2P node on {}", addr);
    info!("Connecting to {} peers: {:?}", peers.len(), peers);

    let store: Store = Arc::new(RwLock::new(HashMap::new()));
    let svc = KvSvc {
        store: store.clone(),
    };

    // spawn server with reflection
    let reflection = tonic_reflection::server::Builder::configure()
        .register_encoded_file_descriptor_set(include_bytes!("../proto/kv_descriptor.bin"))
        .build()?;

    let server = Server::builder()
        .add_service(KvServer::new(svc))
        .add_service(reflection)
        .serve(addr);

    // spawn one background task per peer
    for target in peers {
        let _store = store.clone();
        task::spawn(async move {
            info!("Attempting to connect to peer: {}", target);
            let mut client =
                match kv::kv_client::KvClient::connect(format!("http://{target}")).await {
                    Ok(client) => {
                        info!("Successfully connected to peer: {}", target);
                        client
                    }
                    Err(e) => {
                        error!("Failed to connect to peer {}: {}", target, e);
                        return;
                    }
                };

            // demo: periodically PUT our node-stamp so others can GET it
            loop {
                let key = format!("heartbeat:{}", target);
                let val = format!("{:?}", std::time::SystemTime::now()).into_bytes();

                match client
                    .put(PutRequest {
                        key: key.clone(),
                        value: val,
                    })
                    .await
                {
                    Ok(_) => debug!("Sent heartbeat to {}", target),
                    Err(e) => {
                        warn!("Failed to send heartbeat to {}: {}", target, e);
                        // Try to reconnect on next iteration
                        break;
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }

            warn!("Connection to peer {} lost, task ending", target);
        });
    }

    info!("Server listening on {}", addr);
    server.await?;

    Ok(())
}
