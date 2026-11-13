use crate::account::AccountCommand;
use crate::accounts::AccountsCore;
use crate::error::AccountsError;
use crate::id::AccountId;
use crate::picture::PictureId;

impl AccountsCore {
    /// Shows another image as the account's picture.
    ///
    /// `drop_previous` asks for the image shown before to be deleted, where nothing else uses it.
    /// The assets feature does the deleting when it hears of the change.
    pub async fn change_picture(
        &self,
        account: &AccountId,
        picture: &str,
        drop_previous: bool,
    ) -> Result<(), AccountsError> {
        let picture = PictureId::new(picture)?;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::ChangePicture {
                    picture: picture.clone(),
                    drop_previous,
                },
                &account.acting(),
            )
            .await
        })
    }

    /// Shows no picture any more.
    ///
    /// `drop` asks for the image to be deleted, where nothing else uses it.
    /// Removing a picture the account does not show changes nothing.
    pub async fn remove_picture(
        &self,
        account: &AccountId,
        drop: bool,
    ) -> Result<(), AccountsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            self.commanded_in(
                transaction,
                account,
                None,
                AccountCommand::RemovePicture { drop },
                &account.acting(),
            )
            .await
        })
    }
}
