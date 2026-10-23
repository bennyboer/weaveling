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
theme-chosen = Theme: { $theme }

language = Language
language-machine-translated = { $language } (machine-translated)

workspace-heading = Your projects
workspace-tagline = Bring us your tiny, fragile story ideas, and we will help you weave them into a full epic.
loading = Loading…
workspace-empty = No projects yet. Every epic starts as a little mess.

new-project-working-title = A working title…
new-project-create = Create
new-project-dialog = New project
new-project-confirm = Create project

project-more-actions = More actions
project-rename = Rename
project-delete = Delete
project-rename-save = Save
project-rename-cancel = Cancel

delete-project-dialog = Delete this project?
delete-project-fate = “{ $name }” will be gone for good. Deleting cannot be undone.
delete-project-cancel = Cancel
delete-project-confirm = Delete

new-project-cancel = Cancel

details-title = Title
details-book-language = Language of the book
details-book-language-unset = Not set
details-book-language-hint = The language the book is written in, used wherever its text depends on it.
details-quotation = Quotation marks
details-quotation-as-the-language-has-it = As the language has it, { $marks }
details-quotation-hint = The marks around what is said, put in as " is typed.
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
        [appearances] These appearances were changed elsewhere meanwhile. Close this and open it again to see how they stand.
        [board] This board was changed elsewhere meanwhile. Close this and open it again to see how it stands.
        [idea] This idea was changed elsewhere meanwhile. Close this and open it again to see how it stands.
        [outline] This outline was changed elsewhere meanwhile. Close this and open it again to see how it stands.
        [project] This project was changed elsewhere meanwhile. Close this and open it again to see how it stands.
        [refusal] This refusal was changed elsewhere meanwhile. Close this and open it again to see how it stands.
        [scene] This scene was changed elsewhere meanwhile. Close this and open it again to see how it stands.
        [word-counts] These word counts were changed elsewhere meanwhile. Close this and open it again to see how they stand.
       *[other] This was changed elsewhere meanwhile. Close this and open it again to see how it stands.
    }
problem-overtaken = Someone else changed this meanwhile, so that could not be done.
problem-offline = Could not reach the server. Please try again in a moment.
problem-unexpected = Something unexpected went wrong. Please try again.
problem-dismiss = Dismiss

refused-project-id-invalid = That is not a valid project ID.
refused-project-name-blank = A project needs a title.
refused-project-name-control-character = A title cannot hold control characters.
refused-project-name-too-long = A title can be at most { $max } characters long; this one has { $actual }.
refused-book-language-not-a-tag = “{ $tag }” is not a language tag such as “de”, “en” or “en-GB”.
refused-quotation-marks-unknown = “{ $name }” are not quotation marks Weaveling knows.
refused-project-description-control-character = A description cannot hold control characters other than line breaks.
refused-project-description-too-long = A description can be at most { $max } characters long; this one has { $actual }.
refused-project-deleted = This project was deleted meanwhile.
refused-board-id-invalid = That is not a valid board ID.
refused-idea-already-pinned = This idea is already on the board.
refused-idea-not-pinned = This idea is not on the board.
refused-card-shapeless = A card needs a width and a height.
refused-board-discarded = This board was discarded meanwhile.
refused-idea-id-invalid = That is not a valid idea ID.
refused-idea-title-control-character = An idea's title cannot hold control characters.
refused-idea-title-too-long = An idea's title can be at most { $max } characters long; this one has { $actual }.
refused-idea-discarded = This idea was discarded meanwhile.
refused-outline-id-invalid = That is not a valid outline ID.
refused-section-title-control-character = A section's title cannot hold control characters.
refused-section-title-too-long = A section's title can be at most { $max } characters long; this one has { $actual }.
refused-outline-holds-no-such-kind = An outline holds scenes and ideas, and nothing else.
refused-section-not-found = This section is no longer in the outline.
refused-section-would-contain-itself = A section cannot be moved inside itself.
refused-section-neighbour-not-found = There is nothing at that place to sit after.
refused-not-in-the-outline = This is not in the outline.
refused-outline-discarded = This outline was discarded meanwhile.
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
inspector-appears-nowhere = Not in the book yet — no section notes it, no scene draws on it.
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
outline-how-to-place = Drag onto a section, or click it and then click where it goes. A scene becomes the book; an idea is a note beside it.
renamed-meanwhile = Renamed meanwhile
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
quiet-rearranged = Someone else rearranged the book around “{ $section }”.
quiet-removed = Someone else removed “{ $section }”.
quiet-overtaken = Someone else changed the book meanwhile, so that could not be done. It shows the book as it is now.
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
removed-meanwhile = Removed meanwhile
removed-meanwhile-told-unnamed = Someone else removed “{ $section }”. Your scene is outside the book now, among the scenes not in it.
removed-meanwhile-told = Someone else removed “{ $section }”. Your scene “{ $scene }” is outside the book now, among the scenes not in it.
place-gone = Its place is gone
place-gone-scene = Someone else removed where your new scene was going. It begins “{ $begins }”.
place-gone-section = Someone else removed what your new section “{ $section }” was going after.
renamed-meanwhile-heading-told = While you were editing this heading, someone else renamed it to “{ $theirs }”. Yours is “{ $mine }”.
asked-remove-section = Remove this section?
asked-remove-section-told = Its scenes leave the book but are not deleted: they wait among the scenes not in the book. Sections inside it move up to take its place.
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
chosen-whole = Each section goes with the sections inside it and all their scenes.
chosen-ways-book = Taken out of the book, the scenes wait among those not in it. Deleted for good, they and everything written in them are gone, and that cannot be undone.
chosen-ways-loose = Deleted for good, they and everything written in them are gone, and that cannot be undone.
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
refusals-told = These were saved, but did not reach everywhere they should have. Try again once whatever stopped them is fixed, or acknowledge them to quiet the alarm.
refusals-none = Nothing is left to look at.
refusals-list = Refused changes
close = Close
refusal-wordless = A change did not reach everywhere it should have.
refusal-given-up = Given up on { $when }
refusal-details = Details
refusal-listener = Listener
refusal-message = Message
refusal-attempts = Attempts
refusal-reason = Reason
refusal-retry = Try again
refusal-acknowledge = Acknowledge
alarm-note = Some changes could not be applied.
alarm-show = Show
missing = There is nothing woven at this address.
missing-back = Back to your projects

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

sign-in-heading = Sign in
sign-in-email = Email address
sign-in-password = Password
sign-in-confirm = Sign in
sign-out = Sign out
refused-email-malformed = That is not an email address such as name@example.com.
refused-email-too-long = An email address can be at most { $max } characters long; this one has { $actual }.
refused-account-name-blank = An account needs a name.
refused-account-name-control-character = A name cannot hold control characters.
refused-account-name-too-long = A name can be at most { $max } characters long; this one has { $actual }.
refused-password-too-short = A password needs at least { $min } characters; this one has { $actual }.
refused-password-too-long = A password can be at most { $max } characters long; this one has { $actual }.
refused-verification-code-wrong = That is not the code we sent.
refused-verification-code-expired = This code has run out. Ask for a new one.
refused-verification-code-exhausted = This code was tried too often. Ask for a new one.
refused-sign-in-refused = This email address and password do not sign in.
refused-address-not-verified = This email address is not confirmed yet. Enter the code we sent to it.
refused-not-signed-in = Please sign in first.
sign-up-heading = Sign up
sign-up-name = Your name
sign-up-password-hint = At least 12 characters.
sign-up-confirm = Sign up
sign-up-instead = No account yet? Sign up
sign-in-instead = Already have an account? Sign in
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
operator-heading = Manage instance
operator-you-are = You are the operator of this Weaveling instance. The accounts will be listed here.
operator-become-heading = Become the operator
operator-become-hint = Enter the code the server wrote to its log when it started.
operator-code = Code
operator-become-confirm = Become the operator
operator-no-accounts = This Weaveling instance keeps no accounts, so it has no operator.
