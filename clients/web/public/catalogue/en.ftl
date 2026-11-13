masthead-all-projects = All projects
masthead-views = Views
view-write = Write
view-board = Board
view-outline = Outline
view-ideas = Ideas
status-bar = Status

theme = Theme
theme-light = Light
theme-system = Follow the system
theme-dark = Dark

language = Language
masthead-more = More
masthead-this-project = This project
language-machine-translated = { $language } (machine-translated)

workspace-heading = Your projects
workspace-tagline = Bring us your tiny, fragile story ideas, and we will help you weave them into a full epic.
loading = Loading…
workspace-empty = No projects yet. Every epic starts as a little mess.
workspace-trash-link = Trash, { $count } { $count ->
        [one] project
       *[other] projects
    }

new-project-working-title = A working title…
new-project-create = Create
new-project-dialog = New project
new-project-confirm = Create project
new-project-setting-up = Setting up your project…

project-more-actions = More actions
project-rename = Rename
project-trash = Move to trash
project-rename-save = Save
project-rename-cancel = Cancel

trash-project-dialog = Move this project to the trash?
trash-project-fate = “{ $name }” can be restored from the trash for 30 days. Then it is gone for good.
trash-project-cancel = Cancel
trash-project-confirm = Move to trash
trash-heading = Trash
trash-empty = Nothing is in the trash.
trash-kept-until = Can be restored until { $moment }
trash-restore = Restore

new-project-cancel = Cancel

details-title = Title
details-book-language = Language of the book
details-book-language-unset = Not set
details-book-language-hint = The language your book is written in. Hyphenation and quotation marks follow it.
details-quotation = Quotation marks
details-quotation-as-the-language-has-it = The language's default, { $marks }
details-quotation-hint = The marks placed around dialogue when you type ".
details-description = Description
details-description-placeholder = What the book is about, in a few lines. Optional.

book-language-de = German (Deutsch)
book-language-en = English
book-language-fr = French (Français)
book-language-es = Spanish (Español)
book-language-it = Italian (Italiano)
book-language-nl = Dutch (Nederlands)
book-language-pt = Portuguese (Português)
book-language-pl = Polish (Polski)
book-language-sv = Swedish (Svenska)
book-language-da = Danish (Dansk)
book-language-nb = Norwegian (Norsk)
book-language-fi = Finnish (Suomi)
book-language-cs = Czech (Čeština)

project-details = Project details
project-details-cancel = Cancel
project-details-save = Save
project-details-changing-the-marks = Changing the quotation marks from { $from } to { $to } rewrites them in every scene of the book. In a long book that takes a while.
project-details-back = Back
project-details-change-the-marks = Change the marks

restyling = Rewriting the quotation marks
restyling-waiting = Rewriting the quotation marks…
restyling-running = Rewriting the quotation marks: { $done } of { $total } { $total ->
        [one] scene
       *[other] scenes
    }.
restyling-finished = Every scene now quotes { $marks }.
restyling-close = Close

problem-not-found =
    { $subject ->
        [account] That account no longer exists.
        [appearances] Those appearances no longer exist.
        [board] That board no longer exists.
        [idea] That idea no longer exists.
        [outline] That outline no longer exists.
        [project] That project no longer exists.
        [refusal] That refusal no longer exists.
        [scene] That scene no longer exists.
        [word-counts] Those word counts no longer exist.
       *[other] That no longer exists.
    }
problem-outdated =
    { $subject ->
        [appearances] These appearances were changed by someone else in the meantime. Close this and open it again to see the latest version.
        [board] This board was changed by someone else in the meantime. Close this and open it again to see the latest version.
        [idea] This idea was changed by someone else in the meantime. Close this and open it again to see the latest version.
        [outline] This outline was changed by someone else in the meantime. Close this and open it again to see the latest version.
        [project] This project was changed by someone else in the meantime. Close this and open it again to see the latest version.
        [refusal] This refusal was changed by someone else in the meantime. Close this and open it again to see the latest version.
        [scene] This scene was changed by someone else in the meantime. Close this and open it again to see the latest version.
        [word-counts] These word counts were changed by someone else in the meantime. Close this and open it again to see the latest version.
       *[other] This was changed by someone else in the meantime. Close this and open it again to see the latest version.
    }
problem-overtaken = Someone else changed this in the meantime, so that could not be done.
problem-offline = Could not reach the server. Please try again in a moment.
problem-unexpected = Something unexpected went wrong. Please try again.
problem-dismiss = Dismiss

refused-project-id-invalid = That is not a valid project ID.
refused-project-name-blank = A project needs a title.
refused-project-name-control-character = A title cannot hold control characters.
refused-project-name-too-long = A title can be at most { $max } characters long; this one has { $actual }.
refused-project-not-yet-open = Your project was created but cannot be opened yet. It will appear in your list in a moment.
refused-book-language-not-a-tag = “{ $tag }” is not a language tag such as “de”, “en” or “en-GB”.
refused-quotation-marks-unknown = Weaveling does not know the quotation marks “{ $name }”.
refused-project-description-control-character = A description cannot hold control characters other than line breaks.
refused-project-description-too-long = A description can be at most { $max } characters long; this one has { $actual }.
refused-project-deleted = This project has been deleted in the meantime.
refused-project-trashed = This project has been moved to the trash in the meantime.
refused-project-not-trashed = This project is not in the trash.
refused-not-an-owner = Only an owner of this project may do this.
refused-last-owner = A project keeps at least one owner. Make someone else an owner first.
refused-not-a-member = This person is not a member of this project.
refused-role-unknown = There is no such role in a project.
refused-member-leaving = This account is about to be deleted, so its role cannot be changed.
refused-not-an-author = Only an author of this project may do this.
refused-board-id-invalid = That is not a valid board ID.
refused-idea-already-pinned = This idea is already on the board.
refused-idea-not-pinned = This idea is not on the board.
refused-card-shapeless = A card needs a width and a height.
refused-board-discarded = This board has been discarded in the meantime.
refused-idea-id-invalid = That is not a valid idea ID.
refused-idea-title-control-character = An idea's title cannot hold control characters.
refused-idea-title-too-long = An idea's title can be at most { $max } characters long; this one has { $actual }.
refused-idea-discarded = This idea has been discarded in the meantime.
refused-outline-id-invalid = That is not a valid outline ID.
refused-section-title-control-character = A section's title cannot hold control characters.
refused-section-title-too-long = A section's title can be at most { $max } characters long; this one has { $actual }.
refused-outline-holds-no-such-kind = An outline holds scenes and ideas, and nothing else.
refused-section-not-found = This section is no longer in the outline.
refused-section-would-contain-itself = A section cannot be moved inside itself.
refused-section-neighbour-not-found = The item this should follow is no longer there.
refused-not-in-the-outline = This is not in the outline.
refused-outline-discarded = This outline has been discarded in the meantime.
refused-scene-id-invalid = That is not a valid scene ID.
refused-scene-title-control-character = A scene's title cannot hold control characters.
refused-scene-title-too-long = A scene's title can be at most { $max } characters long; this one has { $actual }.
refused-quotation-never-restyled = The quotation marks of this book were never rewritten.

untitled = Untitled
board = Board
board-not-on-it = Not on the board · { $count }
board-waiting = Ideas not on the board
board-every-idea-pinned = Every idea is on the board.
board-how-to-pin = Click an idea to pin it where there is room.
board-pin = Pin { $idea }
board-actions = Actions for { $idea }
board-rename = Rename { $idea }
board-open = Open { $idea }
board-unpin = Unpin { $idea }
idea-asked = What is the idea?
board-zoom = Zoom
board-zoom-out = Zoom out
board-zoom-reset = Reset the zoom
board-zoom-in = Zoom in
tray = Tray
inspector = Inspector
inspector-idea-name = Idea name
inspector-appears-in = Appears in
inspector-appears-nowhere = Not in the book yet. No section or scene uses it.
board-resize =
    { $side ->
        [top] Resize { $idea } from the top
        [bottom] Resize { $idea } from the bottom
        [left] Resize { $idea } from the left
        [right] Resize { $idea } from the right
        [top-left] Resize { $idea } from the top-left
        [top-right] Resize { $idea } from the top-right
        [bottom-left] Resize { $idea } from the bottom-left
        [bottom-right] Resize { $idea } from the bottom-right
       *[other] Resize { $idea }
    }
drawer-close =
    { $drawer ->
        [tray] Close the tray
        [contents] Close the contents
       *[other] Close
    }
drawer-resize =
    { $drawer ->
        [tray] Resize the tray
        [contents] Resize the contents
       *[other] Resize
    }

scene-empty = Empty
outline-manuscript = Manuscript
outline-the-manuscript = The manuscript
outline-nothing-yet = Nothing in the book yet. Add a section to begin.
outline-add-section = Add a section
outline-not-in-the-book = Not in the book · { $count }
outline-scenes = Scenes
outline-ideas = Ideas
outline-fold = Fold { $section }
outline-section = Section { $section }
outline-actions = Actions for { $section }
outline-move = Move { $section }
outline-move-earlier = Move earlier { $section }
outline-move-later = Move later { $section }
outline-promote = Promote { $section }
outline-demote = Demote { $section }
outline-write-in = Write in { $section }
outline-remove = Remove { $section }
outline-take-out = Take { $name } out of the book
outline-scenes-waiting = Scenes not in the book
outline-every-scene-placed = Every scene has a place in the book.
outline-ideas-waiting = Ideas not in the book
outline-place = Place { $name }
outline-how-to-place = Drag it onto a section, or click it and then click where it should go. A scene becomes part of the book. An idea becomes a note next to it.
renamed-meanwhile = Renamed in the meantime
renamed-meanwhile-told = While you were editing this title, someone else renamed it to “{ $theirs }”. Yours is “{ $mine }”.
renamed-meanwhile-theirs = Take theirs
renamed-meanwhile-mine = Keep mine

ideas = Ideas
ideas-capture = Capture
ideas-empty = No ideas yet. Shoot an idea in and see where it goes.
idea-opening = Opening…

book = The book
writing-sides = Sides of the work
writing-side-book = The book
writing-side-loose = Not in the book
writing-every-scene-placed = Every scene is in the book.
heading-untitled = Untitled section
heading-first = Your book's first section
heading-next = A new section
scene-invitation = An empty scene. Start writing…
scene-next = A new scene
scene-an-empty-one = An empty scene
status-focus = Focus mode
status-focus-hint = Focus mode (Ctrl+Shift+F)
writing-settings = Writing settings
count-whole-book = Count the whole book
count-section = Count the section being written
digit-group = { "," }
words =
    { $count ->
        [one] { $shown } word
       *[other] { $shown } words
    }
words-not-in-the-book = { $words } not in the book
words-in-the-book = { $words } in the book
words-in-section = { $words } in “{ $section }”
words-in-this-section = { $words } in this section
saving = Saving…
all-saved = All saved
setting-spelling = Check spelling
setting-spelling-hint = Underlines words the browser's dictionary does not know.
setting-justified = Justified text
setting-justified-hint = Sets lines flush on both sides like a printed page, hyphenating words in the book's language.
setting-typewriter = Typewriter scrolling
setting-typewriter-hint = Keeps the line you are writing in the middle of the window.
settings-close = Close
contents = Contents
this-scene = This scene
where-you-are = Where you are
path = Path
path-home = Start
path-invitation = Invitation
path-missing = Not found
quiet-rearranged = Someone else rearranged the book around “{ $section }”.
quiet-removed = Someone else removed “{ $section }”.
quiet-overtaken = Someone else changed the book in the meantime, so that could not be done. You now see the current version.
going-back = Back to where you left off

move-scene = Move scene
drag-to-move = Drag to move
scene-actions = Scene actions
menu-rename = Rename
menu-take-out = Take out of the book
menu-bring-in = Bring into the book…
menu-delete-scene = Delete scene
section-actions = Section actions
menu-nest = Nest under the previous section
menu-lift = Lift out a level
menu-remove-section = Remove section
add-here = Add here
menu-new-scene = New scene
menu-new-section = New section
menu-new-top-level-section = New top-level section
menu-bring-back = Bring back a scene…
scene-title = Scene title
menu-button-title = { $menu } ({ $keys })
keys-tab = Tab
keys-shift-tab = Shift+Tab
keys-shift-f10 = Shift+F10
cancel = Cancel
asked-keep-writing = Keep writing
answer-delete = Delete
answer-remove = Remove
answer-show-it = Show it
answer-discard = Discard
answer-put-at-end = Put it at the end of the book
answer-take-out = Take out of the book
answer-delete-for-good = Delete for good
asked-delete-scene = Delete this scene?
asked-delete-scene-told-unnamed = This scene and everything written in it will be gone for good. Deleting cannot be undone.
asked-delete-scene-told = “{ $scene }” and everything written in it will be gone for good. Deleting cannot be undone.
removed-meanwhile = Removed in the meantime
removed-meanwhile-told-unnamed = Someone else removed “{ $section }”. Your scene is now among the scenes not in the book.
removed-meanwhile-told = Someone else removed “{ $section }”. Your scene “{ $scene }” is now among the scenes not in the book.
place-gone = Its place is gone
place-gone-scene = Someone else removed the place your new scene was going to. It begins “{ $begins }”.
place-gone-section = Someone else removed the item your new section “{ $section }” was going to follow.
renamed-meanwhile-heading-told = While you were editing this heading, someone else renamed it to “{ $theirs }”. Yours is “{ $mine }”.
asked-remove-section = Remove this section?
asked-remove-section-told = Its scenes are taken out of the book, but not deleted. You will find them among the scenes not in the book. Sections inside it move up a level.
asked-remove-chosen = Remove what is chosen?
chosen-scenes =
    { $scenes ->
        [one] { $scenes } scene is chosen.
       *[other] { $scenes } scenes are chosen.
    }
chosen-sections =
    { $sections ->
        [one] { $sections } section is chosen.
       *[other] { $sections } sections are chosen.
    }
chosen-both =
    { $scenes ->
        [one] { $scenes } scene
       *[other] { $scenes } scenes
    } and { $sections ->
        [one] { $sections } section
       *[other] { $sections } sections
    } are chosen.
chosen-whole = Each section is removed together with the sections inside it and all their scenes.
chosen-ways-book = If you take them out of the book, the scenes move to the scenes not in the book. If you delete them, they and everything written in them are gone for good. This cannot be undone.
chosen-ways-loose = If you delete them, they and everything written in them are gone for good. This cannot be undone.
placing = Bring into the book
placing-told-unnamed = The scene goes at the end of the section you choose.
placing-told = “{ $scene }” goes at the end of the section you choose.
placing-sections = Sections of the book
bring-back = Bring back a scene
looking = Looking…
bring-back-none = Every scene is in the book already.
bring-back-told = These scenes are not in the book. The one you choose goes in where you clicked.
bring-back-find = Find a scene
bring-back-no-match = No scene matches.

contents-unfold-all = Unfold all
contents-fold-all = Fold all
contents-unfold = Unfold { $section }
this-section = This section
inspector-nothing-yet = Put the cursor in a scene or a heading to see it here.
inspector-section-removed = This section was removed.
pinned-ideas = Pinned ideas
pinned-none = No ideas pinned here.
pin-an-idea = Pin an idea
section-unpin = Unpin { $idea }
drawn-from = Drawn from
drawn-from-none = No ideas linked yet.
link-an-idea = Link an idea
unlink-idea = Unlink { $idea }
pin-ideas = Pin ideas
ideas-to-pin = Ideas to pin
every-idea-pinned = Every idea in this project is already pinned in the outline.
pin-ideas-confirm =
    { $count ->
        [0] Pin
        [1] Pin
       *[other] Pin { $count }
    }
link-ideas = Link ideas
ideas-to-link = Ideas to link
every-idea-linked = Every idea in this project is already linked.
link-ideas-confirm =
    { $count ->
        [0] Link
        [1] Link
       *[other] Link { $count }
    }
find-an-idea = Find an idea
no-idea-matches = No idea matches.

refusals = Changes that could not be applied
refusals-told = These changes were saved, but not every part of Weaveling received them. Fix the cause and try again, or acknowledge them to silence the alarm.
refusals-none = Nothing is left to look at.
refusals-list = Refused changes
close = Close
refusal-wordless = A change did not reach every part of Weaveling.
refusal-given-up = Given up on { $when }
refusal-details = Details
refusal-listener = Listener
refusal-message = Message
refusal-attempts = Attempts
refusal-reason = Reason
refusal-retry = Try again
refusal-too-old = This change is more than 30 days old and is not tried again, because what it was about may no longer exist. Acknowledge it to silence the alarm.
refusal-acknowledge = Acknowledge
alarm-note = Some changes could not be applied.
alarm-show = Show
missing = There is nothing woven at this address.

moment =
    { $day } { $month ->
        [1] Jan
        [2] Feb
        [3] Mar
        [4] Apr
        [5] May
        [6] Jun
        [7] Jul
        [8] Aug
        [9] Sep
        [10] Oct
        [11] Nov
       *[12] Dec
    } { $year }, { $hour }:{ $minute }

day =
    { $day } { $month ->
        [1] Jan
        [2] Feb
        [3] Mar
        [4] Apr
        [5] May
        [6] Jun
        [7] Jul
        [8] Aug
        [9] Sep
        [10] Oct
        [11] Nov
       *[12] Dec
    } { $year }

deletion-pending-heading = Your account is to be deleted
deletion-pending-asked = You asked for it on { $asked }. On { $until }, your account and the projects that only you are a member of will be deleted for good.
deletion-pending-nothing-lost = Until then, nothing is lost. If you keep your account, you stay signed in and find every project as you left it.
deletion-pending-keep = Keep my account
deletion-pending-let-go = Sign out, and let it be deleted
deletion-pending-by-the-operator = The operator of this Weaveling instance has scheduled your account for deletion on { $until }. Ask them if you want to keep it.
deletion-pending-sign-out = Sign out
your-account-heading = Your account
your-account-security = How you sign in:
your-account-delete-heading = Delete this account
your-account-delete-hint = Your account and everything that belongs to it are deleted for good after 30 days.
your-account-delete = Delete this account…
your-account-delete-question = Delete your account?
your-account-delete-kept = You are signed out everywhere right away. Your account and your work are kept for 30 days, until { $until }. Sign in before then to keep everything. After that, it is gone for good.
your-account-delete-alone = Projects that only you are a member of are deleted too.
your-account-delete-password = Your password, to be sure it’s you
your-account-keep = Keep my account
your-account-delete-confirm = Delete my account
your-account-hand-over-heading = Hand these projects over first
your-account-hand-over-hint = You are the only owner of projects that others are members of too. Make one of them an owner, or delete the project. Then you can delete your account.
your-account-hand-over-members = Members
your-account-hand-over-close = Close

sign-in-heading = Sign in
sign-in-email = Email address
sign-in-password = Password
sign-in-confirm = Sign in
passkey-sign-in = Sign in with a passkey
passkey-or-password = or with your password
passkey-not-touched = No passkey was used. Try once more, or sign in with your password.
passkey-failed = Your browser could not ask for a passkey. Sign in with your password.
sign-out = Sign out
refused-email-malformed = That is not an email address such as name@example.com.
refused-email-too-long = An email address can be at most { $max } characters long; this one has { $actual }.
refused-account-name-blank = An account needs a name.
refused-account-name-control-character = A name cannot hold control characters.
refused-account-name-too-long = A name can be at most { $max } characters long; this one has { $actual }.
refused-password-too-short = A password needs at least { $min } characters; this one has { $actual }.
refused-password-too-long = A password can be at most { $max } characters long; this one has { $actual }.
refused-verification-code-wrong = That is not the code we sent.
refused-verification-code-expired = This code has expired. Ask for a new one.
refused-verification-code-exhausted = This code was tried too often. Ask for a new one.
refused-sign-in-refused = The email address or the password is wrong.
refused-address-not-verified = This email address is not confirmed yet. Enter the code we sent to it.
refused-not-signed-in = Please sign in first.
sign-up-heading = Sign up
sign-up-name = Your name
sign-up-password-hint = At least 12 characters.
sign-up-confirm = Sign up
sign-up-instead = No account yet? Sign up
sign-in-instead = Already have an account? Sign in
set-up-instead = Set up this Weaveling instance
set-up-heading = Set up this Weaveling instance
set-up-hint = This Weaveling instance has no operator yet. Sign up with the code from the server's log. Once your address is confirmed, you are its operator.
set-up-code = Code from the server's log
set-up-done = This Weaveling instance is already set up and has an operator. An operator can make you an operator too.
invitation-accept-heading = You have been invited
invitation-accept-hint = Choose your name and a password. Your account uses the email address the invitation came to.
invitation-signed-in = You are signed in as { $email }. Accept the invitation to join the project it is for.
invitation-accept = Accept
invitation-accepted = Invitation accepted. The project will appear in your list in a moment.
invitation-decline = Decline
invitation-declined = You turned down the invitation.
invitation-decline-instead = Decline the invitation instead
invitation-declined-heading = Invitation declined
invitation-declined-hint = You declined it. The person who invited you will see that, and the link no longer works.
notifications = Notifications
notifications-waiting = Notifications: { $count } new
notifications-none = Nothing new just now.
notifications-invitations = Invitations
notice-invitation-from = From { $by }
notice-invitation-unnamed-project = a project
notice-invitation-join = Join
notice-invitation-decline = Decline
notice-invitation-joined = You joined { $project }. It will appear in your list in a moment.
notice-invitation-declined = You turned down the invitation to { $project }.
notifications-roles = Roles
notice-role-unnamed-project = a project
notice-role-changed-by = { $by } made you { $role ->
        [owner] an owner
        [author] an author
       *[reader] a reader
    } of { $project }.
notice-role-changed = You are now { $role ->
        [owner] an owner
        [author] an author
       *[reader] a reader
    } of { $project }.
code-heading = Confirm your address
code-sent = We sent a code to { $email }.
code-field = Code
code-confirm = Confirm
code-again = Send a new code
code-sent-again = A new code is on its way.
code-confirmed = Your address is confirmed. You can sign in now.
code-enter = Enter the code
unreachable-heading = The server is out of reach
try-again = Try again
account-menu = Account: { $name }
reset-instead = Forgot your password?
reset-heading = Set a new password
reset-ask = Send a code
reset-password = New password
reset-confirm = Set the new password
reset-done = Your new password is set. You can sign in with it now.
back-to-sign-in = Back to signing in
refused-too-many-tries =
    { $minutes ->
        [one] Too many tries. Please wait a minute before trying again.
       *[other] Too many tries. Please wait { $minutes } minutes before trying again.
    }
refused-operator-code-wrong = This is not the code the server logged.
account-menu-operator = Manage instance
path-account = Your account
security-heading = Security
security-hint = How you sign in to this Weaveling instance.
security-second-step = Second step
security-second-step-hint = After your password, Weaveling can ask for a second step, so that a password alone is not enough to sign in.
security-app = Authenticator app
security-app-on-since = On since { $when }
security-app-actions = Actions for the authenticator app
security-codes = Recovery codes
security-codes-left = { $left } of 10 left
security-set-up-app = Set up an authenticator app
security-password = Your password, to be sure it’s you
security-key-named = Security key “{ $name }”
security-key-added = Added { $when }
security-key-actions = Actions for { $name }
security-add-key = Add a security key
key-name = Name
key-name-example = YubiKey blue
key-made-heading = Your key is made
key-made-hint = Your browser has registered it. Give it a name you will recognise. You will use it as the second step after your password.
key-made-dismiss = Don’t add it
key-made-done = Add the key
key-added = The key is added. You were signed out everywhere else.
key-added-with-codes = The key is added, and signing in now asks for a second step. You were signed out everywhere else.
key-not-made = No key was made.
key-held-here = This key is added to your account already.
key-failed = Your browser could not make a key. Try another browser or key.
key-rename = Rename
key-rename-heading = Rename “{ $name }”
key-rename-confirm = Save
key-remove = Remove
key-remove-heading = Remove “{ $name }”?
key-remove-hint = You can no longer sign in with this key, and you are signed out everywhere else.
key-remove-confirm = Remove
key-removed = The key is removed. You were signed out everywhere else.
security-signing-in = Signing in
security-passkey-named = Passkey “{ $name }”
security-add-passkey = Add a passkey
security-welcome = Your account is ready. A passkey or a second step can be added now, or any time later on this page.
security-welcome-later = Not now, take me to my projects
key-promote = Use as passkey
key-promote-heading = Make “{ $name }” a passkey
key-promote-hint = The key is set up again as a passkey, so it can sign you in on its own. Your browser asks you to touch it, and for its PIN or your fingerprint. It then replaces the key as it is now. If it was your only second step, signing in no longer asks for one, your recovery codes stop working, and you are signed out everywhere else.
key-promote-confirm = Make it a passkey
key-promoted = The key is now a passkey and signs you in on its own.
key-demote = Use as second step
key-demote-heading = Use “{ $name }” as a second step
key-demote-hint = The passkey no longer signs you in on its own. Signing in asks for your password first, then for the key. You are signed out everywhere else.
key-demote-confirm = Use as second step
key-demoted = The passkey is now your second step.
key-demoted-with-codes = The passkey is now your second step, and signing in asks for it after your password. You were signed out everywhere else.
two-step-unavailable = This Weaveling instance does not support two-step sign-in yet. Ask its operator to set it up.
two-step-set-up-anew = Set up again
two-step-set-up-heading = Set up two-step sign-in
two-step-set-up-hint = After your password, Weaveling will ask for a code from an app on your phone.
two-step-scan = 1 · Scan this with your authenticator app
two-step-qr = QR code for your authenticator app
two-step-cannot-scan = Can’t scan? Type this key:
two-step-copy-key = Copy key
two-step-code = 2 · Enter the code your app shows
two-step-cancel = Cancel
two-step-confirm = Confirm
two-step-nothing-yet = Nothing changes until this code is confirmed.
two-step-on-now = Two-step sign-in is on. You were signed out everywhere else.
two-step-codes-heading = Keep these recovery codes
two-step-codes-hint = If you lose your phone, you can use each code once instead of the app’s code. They are shown only now.
two-step-codes-download = Download as text
two-step-codes-copy = Copy
two-step-codes-print = Print
two-step-codes-kept = I have kept them somewhere safe
two-step-done = Done
copy-done = Copied
copy-refused = Could not copy
second-step-code = The code your app shows for Weaveling
second-step-confirm = Confirm
second-step-key-touch = Touch your security key.
second-step-key-use = Use the key
second-step-key-not-touched = No key was touched. Try once more, or use another way.
second-step-key-failed = Your browser could not ask for the key. Use another way.
second-step-another-way = Use another way
second-step-first-from-now-on = Use this way first from now on
another-way-heading = Another way
another-way-hint = Choose how to confirm it is you this time.
another-way-key = Security key
another-way-key-hint = Touch your key
another-way-app = Authenticator app
another-way-app-hint = Type the code it shows
another-way-recovery = Recovery code
another-way-recovery-hint = Each code works once
another-way-asked-first = Asked first right now
security-preferred-way = Ask first for
security-preferred-key = A security key
security-preferred-key-hint = Touch the key
security-preferred-app = The authenticator app
security-preferred-app-hint = Type the code it shows
security-preferred-last-used = The one I used last
security-preferred-last-used-hint = Whichever way you used last time, on any device
second-step-recovery-code = One of your recovery codes
second-step-recovery-hint = Each code works once. Upper or lower case, with or without the dash.
second-step-no-codes = No codes left and no phone? The operator of this Weaveling instance can turn two-step sign-in off for you.
second-step-not-you = not you?
second-step-ran-out = Signing in took too long. Please sign in again.
recovery-note = You signed in with a recovery code; { $left } are left. Lost your phone?
recovery-note-last = That was your last recovery code. Create new ones, or set up two-step sign-in again, before you sign out.
recovery-note-set-up-anew = Set up two-step sign-in again
two-step-draw-codes = Create new codes
two-step-draw-codes-hint = The new codes replace the old ones, which then no longer work.
two-step-codes-drawn = New recovery codes were created. The old ones no longer work.
two-step-turn-off = Turn off
two-step-turn-off-hint = Signing in no longer asks for a code from your app, and you are signed out everywhere else. If no key is set up as a second step, your recovery codes stop working too.
two-step-set-up-password-hint = Enter your password first. Then you will see the code to add to your app. Setting it up again replaces the app you use now and your recovery codes.
security-add-passkey-hint = Enter your password first. Then your browser asks for the passkey.
security-add-key-hint = Enter your password first. Then your browser asks for the key.
security-password-go-on = Go on
two-step-turned-off = Two-step sign-in is off. You were signed out everywhere else.
instance-account-two-step = Two-step sign-in
instance-account-ways-in = Ways of signing in
instance-account-actions = Actions for { $name }
instance-ways-dialog = How { $name } signs in
instance-ways-password = Password
instance-ways-password-hint = Set when signing up, changed by a password reset
instance-ways-always-kept = cannot be removed
instance-ways-signs-in-alone = signs in on its own
instance-ways-second-step = second step
instance-ways-remove = Remove
instance-ways-remove-named = Remove { $name }
instance-ways-turn-off = Turn off
instance-ways-turn-off-app = Turn off the app
instance-ways-removing = Removing “{ $name }”
instance-ways-turning-off = Turning off the authenticator app
instance-ways-still-password = { $name } still signs in with the password. They are signed out everywhere.
instance-ways-still-second-step = { $name } still signs in with the password and a second step. They are signed out everywhere.
instance-ways-password-alone = { $name } will sign in with the password only, until they set up a second step again. Their recovery codes stop working, and they are signed out everywhere.
instance-ways-make-sure = Make sure first, outside Weaveling, that it really is them asking. Their account’s history shows that you did this.
instance-ways-keep = Keep it
instance-ways-remove-passkey = Remove the passkey
instance-ways-remove-key = Remove the key
instance-ways-close = Close
operator-heading = Manage instance
operator-you-are = You are the operator of this Weaveling instance.
operator-become-heading = Become the operator
operator-become-hint = Enter the code the server wrote to its log when it started.
operator-code = Code
operator-become-confirm = Become the operator
operator-no-accounts = This Weaveling instance has no accounts, so it has no operator.
refused-account-locked = This account is locked.
refused-not-an-operator = Only the operator of this Weaveling instance may do this.
refused-operator-cannot-be-locked = An operator cannot be locked.
refused-lock-reason-blank = Locking an account needs a reason.
refused-lock-reason-control-character = A reason cannot hold control characters.
refused-lock-reason-too-long = A reason can be at most { $max } characters long; this one has { $actual }.
refused-account-id-invalid = That is not a valid account ID.
refused-account-not-found = There is no such account.
instance-sections = Sections
instance-accounts-heading = Accounts
instance-accounts-more = Show more
instance-account-signed-up = Signed up on { $when }
instance-account-unverified = Address not confirmed
instance-invitations-heading = Invitations
instance-invite-email = Email address
instance-invite-confirm = Invite
instance-invitation-withdraw = Withdraw
instance-invitation-invite-again = Invite again
instance-invitations-none = Nobody is invited just now.
instance-invitations-more = Show more invitations
instance-invitation-issued = Invited on { $when }
instance-invitation-open = Open until { $when }
instance-invitation-expired = Expired on { $when }
instance-invitation-declined-address-has-account = Not needed: this email address already has an account
instance-invitation-declined-turned-down = Declined
instance-account-operator = Operator
instance-account-locked = Locked since { $when }
instance-account-lock = Lock…
instance-account-unlock = Unlock
instance-account-leaving = To be deleted on { $when }
instance-account-delete = Delete account…
instance-account-keep = Keep this account
instance-delete-dialog = Delete { $name }’s account?
instance-delete-hint = { $name } is signed out everywhere and cannot sign in. Their account is kept for 30 days, until { $until }, and you can keep it until then. After that, it is deleted together with the projects that only they are a member of. Make sure first, outside Weaveling, that this is wanted. Their account’s history shows that you did this.
instance-delete-hand-over = { $name } is the last owner of projects that others are members of. They have to hand them over first.
instance-delete-cancel = Cancel
instance-delete-confirm = Delete the account
instance-lock-dialog = Lock { $name }
instance-lock-hint = { $name } is signed out at once and cannot sign in again until the account is unlocked. The reason is stored with the lock and shown here.
instance-lock-reason = Reason
instance-lock-cancel = Cancel
instance-lock-confirm = Lock
refused-last-operator = The last operator cannot give up the role, or nobody could manage this Weaveling instance.
refused-operator-address-not-verified = Only an account whose address is confirmed can become an operator.
refused-locked-cannot-become-operator = A locked account cannot become an operator.
refused-signing-up-by-invitation-only = Signing up to this Weaveling instance is by invitation only.
refused-invitation-invalid = This invitation link is invalid or no longer works.
refused-invitation-expired = This invitation has expired. Ask for a new one.
refused-invitation-paused = The project of this invitation is in the trash. The invitation can be accepted once the project is restored.
refused-email-address-has-account = This email address already has an account. Sign in instead.
refused-invitation-for-another-address = This invitation was sent to another address. Sign in with that address to accept it.
refused-invitation-not-by-an-operator = Signing up to this Weaveling instance takes an invitation from its operator. Ask the operator to invite you.
refused-totp-unavailable = This Weaveling instance does not support two-step sign-in yet. Ask its operator to set it up.
refused-totp-nothing-being-set-up = Setting up two-step sign-in has to begin again.
refused-totp-code-wrong = That is not the code your app shows. Wait for the next one and try again.
refused-recovery-code-wrong = That is not one of your recovery codes, or it was used already.
refused-no-second-factor = This account has no two-step sign-in.
refused-keys-unavailable = This Weaveling instance cannot add keys yet. Ask its operator to set its public URL.
refused-no-key-being-added = Adding the key has to begin again.
refused-no-key-being-asked = Asking for the key has to begin again.
refused-key-refused = Your browser’s answer did not create a key for this Weaveling instance. Try once more.
refused-key-held-already = This key is added to an account already.
refused-no-such-key = This account has no such key.
refused-key-id-invalid = That is not a valid key ID.
refused-key-name-taken = Another of your keys has this name already.
refused-key-name-blank = A key needs a name.
refused-key-name-control-character = A key’s name cannot hold control characters.
refused-key-name-too-long = A key’s name can be at most { $max } characters long; this one has { $actual }.
refused-too-many-keys = An account holds at most 20 keys. Remove one first.
refused-password-wrong = That is not your password.
refused-projects-to-hand-over = You are the last owner of projects that others are members of. Make someone else an owner of each first.
refused-no-deletion-pending = No deletion of this account is pending.
refused-deletion-asked-by-the-operator = The operator of this Weaveling instance asked for this account to be deleted, so only the operator can cancel it.
refused-preferred-way-unknown = Choose the app, a key, or the way you used last.
refused-key-role-unknown = A key is either a passkey or a second step.
instance-account-appoint = Make operator
instance-account-dismiss = Take away the operator role
instance-appoint-dialog = Make { $name } an operator
instance-appoint-hint = { $name } can then manage this Weaveling instance as you do: list the accounts, lock and unlock them, and appoint and dismiss operators.
instance-appoint-confirm = Make operator
instance-dismiss-dialog = Take the operator role from { $name }
instance-dismiss-hint = { $name } can then no longer manage this Weaveling instance.
instance-dismiss-yourself-hint = You can then no longer manage this Weaveling instance, and only another operator can give the role back.
instance-dismiss-confirm = Take away the role
instance-role-cancel = Cancel
refused-public-url-missing = This Weaveling instance does not know where it is served, so its invitations would lead nowhere. Ask its operator to set its public URL.
refused-invitation-id-invalid = That is not a valid invitation ID.
refused-invitation-not-found = There is no such invitation.
refused-invitation-already-accepted = This invitation has already been accepted.
refused-too-late-to-try-again = This change is too old to be tried again.

members-link = Members
members-link-of = Members: { $names }
share = Share
share-title = Share “{ $project }”
share-people = People with access
share-invited = Invited
share-all-members = All members and invitations
members-heading = Members
members-hint = Who may read and write this project.
members-you = (you)
members-unnamed = Not known by name yet
members-role-owner = Owner
members-role-author = Author
members-role-reader = Reader
roles-changed = Your role has been changed to { $role }.
members-roles = Owners invite, change roles and delete the project. Authors read and write. Readers only read.
members-role-of = Role of { $name }
members-remove = Remove
members-leave = Leave
members-sections = Sections
members-people = People
members-invited = Invited
members-invite-as = As
members-only-owner = Owner, the only one
members-last-owner-heading = You are its only owner
members-last-owner-hint = Before you leave, make someone else an owner. Otherwise nobody could invite, change roles or move this project to the trash.
members-new-owner = New owner
members-candidate = { $name } ({ $role })
members-stay = Stay
members-hand-over = Hand over and leave
members-alone-heading = Nobody else is in it
members-alone-hint = This project has no other members, so if you left, nobody would be left in it. Move it to the trash instead, where it can be restored for 30 days, or invite someone and hand it over first.
members-trash-project = Move the project to the trash

decimal-point = { "." }
size-bytes = { $amount }{ "\u00A0" }B
size-kilobytes = { $amount }{ "\u00A0" }KB
size-megabytes = { $amount }{ "\u00A0" }MB
size-gigabytes = { $amount }{ "\u00A0" }GB
size-terabytes = { $amount }{ "\u00A0" }TB

refused-asset-id-invalid = That is not a valid image ID.
refused-asset-not-an-image = This file is not an image Weaveling can use. Upload a JPEG, PNG, GIF or WebP image.
refused-asset-image-unreadable = This image could not be read. It may be damaged.
refused-asset-image-too-large = This image has too many pixels. It can have at most 50 megapixels.
refused-asset-limit-reached =
    { $bound ->
        [upload] An image can be at most { $most } large.
        [instance] This Weaveling instance has no room left for this image. It holds { $used } of { $most }.
        [account] You have no room left for this image. You use { $used } of { $most }.
        [project] This project has no room left for this image. It uses { $used } of { $most }.
       *[other] There is no room left for this image.
    }
refused-asset-no-account = Only a signed-in account can keep images of its own.
refused-asset-limit-uncountable = That limit is too large.
refused-asset-sort-unknown = The list cannot be sorted that way.
refused-asset-cursor-invalid = The list cannot go on from there. Load it again.

your-account-storage = Your media and the storage they use:
storage-heading = Storage
storage-used = { $used } of { $most } used
storage-used-unbounded = { $used } used
storage-hint = All media you uploaded, into projects and as your own. Delete what you no longer need to free up storage.
storage-sort = Sort by
storage-sort-newest = Newest first
storage-sort-largest = Largest first
storage-none = You have not uploaded anything yet.
storage-own = Your own file
storage-in-project = In { $project }
storage-in-a-project = In a project
storage-detail = { $size } · uploaded on { $when }
storage-more = Show more
storage-delete = Delete
storage-not-deletable = Only authors of the project can delete it.
storage-delete-heading = Delete this file?
storage-delete-hint = The file is deleted for good. This cannot be undone.
storage-delete-cancel = Cancel
storage-delete-confirm = Delete the file
