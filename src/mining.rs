use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use rand::{Rng, SeedableRng, rngs::StdRng};
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
    time::interval,
};
use tracing::warn;
use twenty_first::prelude::{BFieldCodec, Digest, Tip5};

use crate::pool::TemplateResponse;

#[derive(Clone)]
pub enum WorkerMessage {
    NewTemplate(Option<TemplateResponse>),
    Shutdown,
}

pub struct Mining {
    worker_channel: broadcast::Sender<WorkerMessage>,
    nonce_channel: mpsc::Sender<(Digest, Digest)>,
    generated_hashes: Arc<AtomicU64>,
}

impl Mining {
    pub fn new(
        worker_channel: broadcast::Sender<WorkerMessage>,
        nonce_channel: mpsc::Sender<(Digest, Digest)>,
    ) -> Self {
        Mining {
            worker_channel,
            nonce_channel,
            generated_hashes: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn spawn_monitoring(&self) -> JoinHandle<()> {
        let generated_hashes = self.generated_hashes.clone();

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(10));
            let mut last_hashes = 0u64;

            loop {
                interval.tick().await;
                let current = generated_hashes.load(Ordering::Relaxed);
                let hashes_per_sec = (current - last_hashes) / 10;
                warn!(
                    "Hash rate: {:.2} kH/s (observed)",
                    hashes_per_sec as f64 / 1_000.0
                );
                last_hashes = current;
            }
        })
    }

    pub fn spawn_worker(&self) {
        let mut worker_channel = self.worker_channel.subscribe();
        let nonce_channel = self.nonce_channel.clone();
        let generated_hashes = self.generated_hashes.clone();

        tokio::spawn(async move {
            let mut rng = StdRng::from_rng(&mut rand::rng());
            let mut current_template: Option<TemplateResponse> = None;
            let mut shutdown = false;

            while !shutdown {
                match worker_channel.try_recv() {
                    Ok(WorkerMessage::NewTemplate(template)) => {
                        current_template = template;
                    }
                    Ok(WorkerMessage::Shutdown) => {
                        shutdown = true;
                        continue;
                    }
                    _ => {}
                }

                if let Some(ref template) = current_template {
                    let nonce = rng.random::<Digest>();

                    let hash = fast_kernel_mast_hash(
                        template.kernel_auth_path,
                        template.header_auth_path,
                        nonce,
                    );

                    if hash <= template.threshold {
                        let _ = nonce_channel.send((template.id, nonce)).await;
                        tokio::task::yield_now().await;
                    }

                    generated_hashes.fetch_add(1, Ordering::Relaxed);
                }
            }
        });
    }

    pub fn terminate_workers(&self) {
        let _ = self.worker_channel.send(WorkerMessage::Shutdown);
    }
}

pub fn fast_kernel_mast_hash(
    kernel_auth_path: [Digest; 2],
    header_auth_path: [Digest; 3],
    nonce: Digest,
) -> Digest {
    let header_mast_hash = Tip5::hash_pair(Tip5::hash_varlen(&nonce.encode()), header_auth_path[0]);
    let header_mast_hash = Tip5::hash_pair(header_mast_hash, header_auth_path[1]);
    let header_mast_hash = Tip5::hash_pair(header_auth_path[2], header_mast_hash);

    Tip5::hash_pair(
        Tip5::hash_pair(
            Tip5::hash_varlen(&header_mast_hash.encode()),
            kernel_auth_path[0],
        ),
        kernel_auth_path[1],
    )
}
