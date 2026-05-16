use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupSurfaceOwnership {
    ContinueSetup,
    FollowUpProduct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SetupBoundaryKind {
    Ask,
    Chat,
    Personalize,
    ChannelReview,
    Doctor,
}

impl SetupBoundaryKind {
    pub const fn ownership(self) -> SetupSurfaceOwnership {
        match self {
            Self::ChannelReview => SetupSurfaceOwnership::ContinueSetup,
            Self::Ask | Self::Chat | Self::Personalize | Self::Doctor => {
                SetupSurfaceOwnership::FollowUpProduct
            }
        }
    }
}
