//! Application state — composition root for services.
//! Business logic lives in `services/*`, not here.

use crate::services::ai::AiService;
use crate::services::audit::AuditService;
use crate::services::connections::ConnectionService;
use crate::services::security::ApprovalStore;

pub struct AppState {
    pub connections: ConnectionService,
    pub audit: AuditService,
    pub approvals: ApprovalStore,
    pub ai: AiService,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            connections: ConnectionService::new(),
            audit: AuditService::new(),
            approvals: ApprovalStore::new(),
            ai: AiService::new(),
        }
    }
}
