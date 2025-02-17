use candid::{CandidType, Deserialize};
use serde::{Serialize};
use ic_cdk_macros::{query, update};
use ic_cdk::api::call::call;
use ic_cdk::export::Principal;
use ic_stable_structures::{StableBTreeMap, Storable, BoundedStorable, memory_manager::VirtualMemory, DefaultMemoryImpl, Memory};
use serde_json::json;
use serde_json::to_vec;
use uuid::Uuid;

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
struct Task {
    id: String,
    description: String,
    status: String, // Pending, Executing, Completed
}

#[derive(CandidType, Deserialize, Serialize, Clone, Debug)]
struct Reference {
    id: String,
    tasks: Vec<Task>,
    zk_proof: Option<String>,
}

impl Storable for Reference {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        std::borrow::Cow::Owned(serde_json::to_vec(self).unwrap())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        serde_json::from_slice(&bytes).unwrap()
    }
}

impl BoundedStorable for Reference {
    const MAX_SIZE: u32 = 4096;
    const IS_FIXED_SIZE: bool = false;
}

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct StorableString(String);

impl Storable for StorableString {
    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        std::borrow::Cow::Owned(self.0.as_bytes().to_vec())
    }

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        StorableString(String::from_utf8(bytes.to_vec()).unwrap())
    }
}

impl BoundedStorable for StorableString {
    const MAX_SIZE: u32 = 256;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static REFERENCES: StableBTreeMap<StorableString, Reference, VirtualMemory<DefaultMemoryImpl>> = 
        StableBTreeMap::new(VirtualMemory::new(DefaultMemoryImpl::default()));
}

#[update]
fn generate_reference() -> String {
    let id = Uuid::new_v4().to_string();
    let new_reference = Reference {
        id: id.clone(),
        tasks: vec![],
        zk_proof: None,
    };
    
    REFERENCES.with(|store| {
        store.insert(StorableString(id.clone()), new_reference);
    });
    id
}

#[update]
fn assign_task(reference_id: String, description: String) -> Option<String> {
    let task_id = Uuid::new_v4().to_string();
    let task = Task {
        id: task_id.clone(),
        description,
        status: "Pending".to_string(),
    };
    
    REFERENCES.with(|store| {
        let reference_option = store.get(&StorableString(reference_id.clone()));
        if let Some(reference) = reference_option {
            let mut reference = reference.clone();
            reference.tasks.push(task.clone());
            store.insert(StorableString(reference_id.clone()), reference);
            Some(task_id)
        } else {
            None
        }
    })
}

#[query]
fn get_tasks(reference_id: String) -> Option<Vec<Task>> {
    REFERENCES.with(|store| {
        store.get(&StorableString(reference_id)).map(|r| r.tasks.clone())
    })
}

#[update]
fn execute_tasks(reference_id: String) -> Option<Vec<Task>> {
    REFERENCES.with(|store| {
        let reference_option = store.get(&StorableString(reference_id.clone()));
        if let Some(reference) = reference_option {
            let mut reference = reference.clone();
            for task in &mut reference.tasks {
                task.status = "Completed".to_string();
            }
            let zk_proof = generate_zk_proof(&reference.id);
            reference.zk_proof = Some(zk_proof.clone());
            execute_on_eth_btc(&reference.id);
            store.insert(StorableString(reference_id.clone()), reference);
            Some(reference.tasks.clone())
        } else {
            None
        }
    })
}

#[update]
fn delete_reference(reference_id: String) -> bool {
    REFERENCES.with(|store| {
        store.remove(&StorableString(reference_id)).is_some()
    })
}

fn generate_zk_proof(reference_id: &str) -> String {
    let proof_input = json!({ "reference_id": reference_id });
    proof_input.to_string()
}

#[update]
async fn execute_on_eth_btc(reference_id: &str) {
    let chain_fusion_principal = Principal::management_canister();
    
    let eth_tx = json!({
        "chain": "Ethereum",
        "action": "execute_task",
        "reference_id": reference_id
    });

    let btc_tx = json!({
        "chain": "Bitcoin",
        "action": "execute_task",
        "reference_id": reference_id
    });

    let eth_payload = to_vec(&eth_tx).unwrap();
    let btc_payload = to_vec(&btc_tx).unwrap();

    let _eth_result: Result<(), _> = call::<(Vec<u8>,), ()>(
        chain_fusion_principal, 
        "execute_transaction", 
        (eth_payload,)
    ).await;
    
    let _btc_result: Result<(), _> = call::<(Vec<u8>,), ()>(
        chain_fusion_principal, 
        "execute_transaction", 
        (btc_payload,)
    ).await;
}
