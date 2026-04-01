use loongclaw_contracts::GovernedSessionMode;

use crate::KernelContext;

#[derive(Clone, Copy, Default)]
pub enum ConversationRuntimeBinding<'a> {
    Kernel(&'a KernelContext),
    #[default]
    AdvisoryOnly,
}

impl<'a> ConversationRuntimeBinding<'a> {
    pub fn from_optional_kernel_context(kernel_ctx: Option<&'a KernelContext>) -> Self {
        match kernel_ctx {
            Some(kernel_ctx) => Self::Kernel(kernel_ctx),
            None => Self::AdvisoryOnly,
        }
    }

    pub fn kernel(kernel_ctx: &'a KernelContext) -> Self {
        Self::Kernel(kernel_ctx)
    }

    pub const fn advisory_only() -> Self {
        Self::AdvisoryOnly
    }

    pub const fn direct() -> Self {
        Self::AdvisoryOnly
    }

    pub fn kernel_context(self) -> Option<&'a KernelContext> {
        match self {
            Self::Kernel(kernel_ctx) => Some(kernel_ctx),
            Self::AdvisoryOnly => None,
        }
    }

    pub const fn is_kernel_bound(self) -> bool {
        matches!(self, Self::Kernel(_))
    }

    pub const fn session_mode(self) -> GovernedSessionMode {
        match self {
            Self::Kernel(_) => GovernedSessionMode::MutatingCapable,
            Self::AdvisoryOnly => GovernedSessionMode::AdvisoryOnly,
        }
    }

    pub const fn allows_mutation(self) -> bool {
        matches!(self, Self::Kernel(_))
    }
}
