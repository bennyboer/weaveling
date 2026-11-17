use assets_contract::{AssetUseDTO, HolderDTO, OwnerDTO, purpose};
use assets_core::{AssetUse, Holder, Owner, Purpose};

pub(crate) fn to_use_dto(used: &AssetUse) -> AssetUseDTO {
    AssetUseDTO {
        purpose: match used.purpose {
            Purpose::AccountPicture => purpose::ACCOUNT_PICTURE,
            Purpose::SceneImage => purpose::SCENE_IMAGE,
        }
        .to_owned(),
        by: match &used.by {
            Holder::Account(account) => HolderDTO::Account(account.to_string()),
            Holder::Scene(scene) => HolderDTO::Scene(scene.to_string()),
        },
    }
}

pub(crate) fn to_owner_dto(owner: &Owner) -> OwnerDTO {
    match owner {
        Owner::Project(project) => OwnerDTO::Project(project.to_string()),
        Owner::Account(account) => OwnerDTO::Account(account.to_string()),
    }
}
