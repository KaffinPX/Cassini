use std::time::Duration;

use crate::mining::WorkerMessage;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::{broadcast, mpsc},
    task::JoinHandle,
    time::interval,
};
use tracing::{error, info};
use twenty_first::prelude::Digest;

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TemplateResponse {
    pub id: Digest,
    pub kernel_auth_path: [Digest; 2],
    pub header_auth_path: [Digest; 3],
    pub threshold: Digest,
}

#[derive(Serialize)]
pub struct WorkRequest {
    pub id: String,
    pub nonce: String,
    pub address: String,
}

#[derive(Deserialize)]
pub struct WorkResponse {
    pub success: bool,
    pub error: Option<String>,
}

pub struct Pool {
    url: String,
    address: String,
}

impl Pool {
    pub fn new(url: String, address: String) -> Self {
        Pool { url, address }
    }

    pub fn spawn_template_refresher(
        &self,
        worker_channel: broadcast::Sender<WorkerMessage>,
    ) -> JoinHandle<()> {
        let endpoint = format!("{}/template", self.url);

        tokio::spawn(async move {
            let mut interval = interval(Duration::from_secs(30));

            loop {
                interval.tick().await;
                let template = match reqwest::get(&endpoint).await {
                    Ok(response) => match response.json::<TemplateResponse>().await {
                        Ok(t) => Some(t),
                        Err(_) => None,
                    },
                    Err(_) => None,
                };
                let _ = worker_channel.send(WorkerMessage::NewTemplate(template));
            }
        })
    }

    pub fn spawn_nonce_processor(
        &self,
        mut nonce_receiver: mpsc::Receiver<(Digest, Digest)>,
    ) -> JoinHandle<()> {
        let endpoint = format!("{}/submit_work", self.url);
        let address = self.address.clone();

        tokio::spawn(async move {
            while let Some((template_id, nonce)) = nonce_receiver.recv().await {
                let template_id = template_id.to_hex();
                info!("Submitting work for template {}...", template_id);

                let work_request = WorkRequest {
                    id: template_id.clone(),
                    nonce: nonce.to_hex(),
                    address: address.clone(),
                };

                match reqwest::Client::new()
                    .post(&endpoint)
                    .json(&work_request)
                    .send()
                    .await
                {
                    Ok(response) => match response.json::<WorkResponse>().await {
                        Ok(result) => {
                            if result.success {
                                info!("Work {} got accepted by pool.", template_id);
                            } else {
                                error!("Work {} rejected: {}", template_id, result.error.unwrap());
                            }
                        }
                        Err(e) => error!("Failed to parse response for {}: {}", template_id, e),
                    },
                    Err(e) => error!("Submission failed for {}: {}", template_id, e),
                }
            }
        })
    }
}
