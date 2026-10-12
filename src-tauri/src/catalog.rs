// 服务目录：与前端共用 src/catalog/services.json，保证展示与部署口径一致
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeDef {
    /// 具名卷（Docker 管理）
    pub name: Option<String>,
    /// 绑定到数据目录下的子目录
    pub bind: Option<String>,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDef {
    pub id: String,
    pub name: String,
    pub engine: String,
    pub image: String,
    pub image_gpu: Option<String>,
    pub build: Option<String>,
    pub docker_hub: bool,
    pub container_port: u16,
    pub host_port: u16,
    pub volumes: Vec<VolumeDef>,
    pub env: BTreeMap<String, String>,
    pub command: Vec<String>,
    /// none / optional / required
    pub gpu: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingModelDef {
    pub name: String,
    pub dims: u32,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogFile {
    services: Vec<ServiceDef>,
    embedding_models: Vec<EmbeddingModelDef>,
}

const RAW: &str = include_str!("../../src/catalog/services.json");

fn catalog() -> &'static CatalogFile {
    static C: OnceLock<CatalogFile> = OnceLock::new();
    C.get_or_init(|| serde_json::from_str(RAW).expect("services.json 格式错误"))
}

pub fn services() -> &'static [ServiceDef] {
    &catalog().services
}

pub fn find(id: &str) -> Option<&'static ServiceDef> {
    services().iter().find(|s| s.id == id)
}

pub fn embedding_models() -> &'static [EmbeddingModelDef] {
    &catalog().embedding_models
}

pub fn container_name(id: &str) -> String {
    format!("knowark-{id}")
}
