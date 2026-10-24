masthead-all-projects = Tous les projets
masthead-views = Vues
view-write = Écrire
view-board = Tableau
view-outline = Plan
view-ideas = Idées
status-bar = État

theme = Thème
theme-light = Clair
theme-system = Comme le système
theme-dark = Sombre
theme-chosen = Thème : { $theme }

language = Langue
language-machine-translated = { $language } (traduction automatique)

workspace-heading = Vos projets
workspace-tagline = Confiez-nous vos petites idées d’histoires, si fragiles, et nous vous aiderons à les tisser en une véritable épopée.
loading = Chargement…
workspace-empty = Pas encore de projet. Toute épopée commence par un petit désordre.

new-project-working-title = Un titre provisoire…
new-project-create = Créer
new-project-dialog = Nouveau projet
new-project-confirm = Créer le projet

project-more-actions = Plus d’actions
project-rename = Renommer
project-delete = Supprimer
project-rename-save = Enregistrer
project-rename-cancel = Annuler

delete-project-dialog = Supprimer ce projet ?
delete-project-fate = « { $name } » disparaîtra pour de bon. La suppression est irréversible.
delete-project-cancel = Annuler
delete-project-confirm = Supprimer

new-project-cancel = Annuler

details-title = Titre
details-book-language = Langue du livre
details-book-language-unset = Non définie
details-book-language-hint = La langue dans laquelle le livre est écrit, utilisée partout où son texte en dépend.
details-quotation = Guillemets
details-quotation-as-the-language-has-it = Ceux de la langue, { $marks }
details-quotation-hint = Les signes autour de ce qui est dit, posés dès que l’on tape ".
details-description = Description
details-description-placeholder = De quoi parle le livre, en quelques lignes. Facultatif.

book-language-de = Allemand (Deutsch)
book-language-en = Anglais (English)
book-language-fr = Français
book-language-es = Espagnol (Español)
book-language-it = Italien (Italiano)
book-language-nl = Néerlandais (Nederlands)
book-language-pt = Portugais (Português)
book-language-pl = Polonais (Polski)
book-language-sv = Suédois (Svenska)
book-language-da = Danois (Dansk)
book-language-nb = Norvégien (Norsk)
book-language-fi = Finnois (Suomi)
book-language-cs = Tchèque (Čeština)

project-details = Détails du projet
project-details-cancel = Annuler
project-details-save = Enregistrer
project-details-changing-the-marks = Passer des guillemets { $from } à { $to } les réécrit dans chaque scène du livre. Dans un long livre, cela prend un moment.
project-details-back = Retour
project-details-change-the-marks = Changer les guillemets

restyling = Réécriture des guillemets
restyling-waiting = Réécriture des guillemets…
restyling-running = Réécriture des guillemets : { $done } sur { $total } { $total ->
        [one] scène
       *[other] scènes
    }.
restyling-finished = Chaque scène emploie désormais { $marks }.
restyling-close = Fermer

problem-not-found =
    { $subject ->
        [account] Ce compte n’existe plus.
        [appearances] Ces apparitions n’existent plus.
        [board] Ce tableau n’existe plus.
        [idea] Cette idée n’existe plus.
        [outline] Ce plan n’existe plus.
        [project] Ce projet n’existe plus.
        [refusal] Ce refus n’existe plus.
        [scene] Cette scène n’existe plus.
        [word-counts] Ces décomptes de mots n’existent plus.
       *[other] Cela n’existe plus.
    }
problem-outdated =
    { $subject ->
        [appearances] Ces apparitions ont été modifiées ailleurs entre-temps. Fermez ceci et rouvrez-les pour voir où elles en sont.
        [board] Ce tableau a été modifié ailleurs entre-temps. Fermez ceci et rouvrez-le pour voir où il en est.
        [idea] Cette idée a été modifiée ailleurs entre-temps. Fermez ceci et rouvrez-la pour voir où elle en est.
        [outline] Ce plan a été modifié ailleurs entre-temps. Fermez ceci et rouvrez-le pour voir où il en est.
        [project] Ce projet a été modifié ailleurs entre-temps. Fermez ceci et rouvrez-le pour voir où il en est.
        [refusal] Ce refus a été modifié ailleurs entre-temps. Fermez ceci et rouvrez-le pour voir où il en est.
        [scene] Cette scène a été modifiée ailleurs entre-temps. Fermez ceci et rouvrez-la pour voir où elle en est.
        [word-counts] Ces décomptes de mots ont été modifiés ailleurs entre-temps. Fermez ceci et rouvrez-les pour voir où ils en sont.
       *[other] Cela a été modifié ailleurs entre-temps. Fermez ceci et rouvrez-le pour voir où il en est.
    }
problem-overtaken = Quelqu’un d’autre l’a modifié entre-temps, cela n’a donc pas pu être fait.
problem-offline = Impossible de joindre le serveur. Veuillez réessayer dans un instant.
problem-unexpected = Une erreur inattendue s’est produite. Veuillez réessayer.
problem-dismiss = Ignorer

refused-project-id-invalid = Ce n’est pas un identifiant de projet valide.
refused-project-name-blank = Un projet a besoin d’un titre.
refused-project-name-control-character = Un titre ne peut pas contenir de caractères de contrôle.
refused-project-name-too-long = Un titre peut compter au plus { $max } caractères ; celui-ci en a { $actual }.
refused-book-language-not-a-tag = « { $tag } » n’est pas une étiquette de langue comme « de », « en » ou « en-GB ».
refused-quotation-marks-unknown = « { $name } » ne sont pas des guillemets que Weaveling connaît.
refused-project-description-control-character = Une description ne peut pas contenir de caractères de contrôle, sauf des sauts de ligne.
refused-project-description-too-long = Une description peut compter au plus { $max } caractères ; celle-ci en a { $actual }.
refused-project-deleted = Ce projet a été supprimé entre-temps.
refused-board-id-invalid = Ce n’est pas un identifiant de tableau valide.
refused-idea-already-pinned = Cette idée est déjà sur le tableau.
refused-idea-not-pinned = Cette idée n’est pas sur le tableau.
refused-card-shapeless = Une carte a besoin d’une largeur et d’une hauteur.
refused-board-discarded = Ce tableau a été abandonné entre-temps.
refused-idea-id-invalid = Ce n’est pas un identifiant d’idée valide.
refused-idea-title-control-character = Le titre d’une idée ne peut pas contenir de caractères de contrôle.
refused-idea-title-too-long = Le titre d’une idée peut compter au plus { $max } caractères ; celui-ci en a { $actual }.
refused-idea-discarded = Cette idée a été abandonnée entre-temps.
refused-outline-id-invalid = Ce n’est pas un identifiant de plan valide.
refused-section-title-control-character = Le titre d’une section ne peut pas contenir de caractères de contrôle.
refused-section-title-too-long = Le titre d’une section peut compter au plus { $max } caractères ; celui-ci en a { $actual }.
refused-outline-holds-no-such-kind = Un plan contient des scènes et des idées, et rien d’autre.
refused-section-not-found = Cette section n’est plus dans le plan.
refused-section-would-contain-itself = Une section ne peut pas être déplacée en elle-même.
refused-section-neighbour-not-found = À cet endroit, il n’y a rien après quoi le placer.
refused-not-in-the-outline = Ceci n’est pas dans le plan.
refused-outline-discarded = Ce plan a été abandonné entre-temps.
refused-scene-id-invalid = Ce n’est pas un identifiant de scène valide.
refused-scene-title-control-character = Le titre d’une scène ne peut pas contenir de caractères de contrôle.
refused-scene-title-too-long = Le titre d’une scène peut compter au plus { $max } caractères ; celui-ci en a { $actual }.
refused-quotation-never-restyled = Les guillemets de ce livre n’ont jamais été réécrits.

untitled = Sans titre
board = Tableau
board-not-on-it = Hors du tableau · { $count }
board-waiting = Idées hors du tableau
board-every-idea-pinned = Toutes les idées sont sur le tableau.
board-how-to-pin = Cliquez sur une idée pour l’épingler là où il y a de la place.
board-pin = Épingler { $idea }
board-actions = Actions pour { $idea }
board-rename = Renommer { $idea }
board-open = Ouvrir { $idea }
board-unpin = Retirer { $idea } du tableau
idea-asked = Quelle est l’idée ?
board-zoom = Zoom
board-zoom-out = Zoom arrière
board-zoom-reset = Réinitialiser le zoom
board-zoom-in = Zoom avant
tray = Volet
inspector = Inspecteur
inspector-idea-name = Nom de l’idée
inspector-appears-in = Apparaît dans
inspector-appears-nowhere = Pas encore dans le livre — aucune section ne la note, aucune scène ne s’en inspire.
board-resize =
    { $side ->
        [top] Redimensionner { $idea } par le haut
        [bottom] Redimensionner { $idea } par le bas
        [left] Redimensionner { $idea } par la gauche
        [right] Redimensionner { $idea } par la droite
        [top-left] Redimensionner { $idea } par le coin supérieur gauche
        [top-right] Redimensionner { $idea } par le coin supérieur droit
        [bottom-left] Redimensionner { $idea } par le coin inférieur gauche
        [bottom-right] Redimensionner { $idea } par le coin inférieur droit
       *[other] Redimensionner { $idea }
    }
drawer-close =
    { $drawer ->
        [tray] Fermer le volet
        [contents] Fermer la table des matières
       *[other] Fermer
    }
drawer-resize =
    { $drawer ->
        [tray] Redimensionner le volet
        [contents] Redimensionner la table des matières
       *[other] Redimensionner
    }

scene-empty = Vide
outline-manuscript = Manuscrit
outline-the-manuscript = Le manuscrit
outline-nothing-yet = Rien dans le livre pour l’instant. Ajoutez une section pour commencer.
outline-add-section = Ajouter une section
outline-not-in-the-book = Hors du livre · { $count }
outline-scenes = Scènes
outline-ideas = Idées
outline-fold = Replier { $section }
outline-section = Section { $section }
outline-actions = Actions pour { $section }
outline-move = Déplacer { $section }
outline-move-earlier = Avancer { $section }
outline-move-later = Reculer { $section }
outline-promote = Remonter { $section } d’un niveau
outline-demote = Descendre { $section } d’un niveau
outline-write-in = Écrire dans { $section }
outline-remove = Supprimer { $section }
outline-take-out = Retirer { $name } du livre
outline-scenes-waiting = Scènes hors du livre
outline-every-scene-placed = Chaque scène a sa place dans le livre.
outline-ideas-waiting = Idées hors du livre
outline-place = Placer { $name }
outline-how-to-place = Faites-la glisser sur une section, ou cliquez dessus puis là où elle va. Une scène devient le livre ; une idée, une note à côté.
renamed-meanwhile = Renommé entre-temps
renamed-meanwhile-told = Pendant que vous modifiiez ce titre, quelqu’un d’autre l’a renommé en « { $theirs } ». Le vôtre est « { $mine } ».
renamed-meanwhile-theirs = Prendre le sien
renamed-meanwhile-mine = Garder le mien

ideas = Idées
ideas-capture = Noter
ideas-empty = Pas encore d’idées. Lancez une idée et voyez où elle mène.
idea-opening = Ouverture…

book = Le livre
writing-sides = Côtés de l’œuvre
writing-side-book = Le livre
writing-side-loose = Hors du livre
writing-every-scene-placed = Toutes les scènes sont dans le livre.
heading-untitled = Section sans titre
heading-first = La première section de votre livre
heading-next = Une nouvelle section
scene-invitation = Une scène vide. Commencez à écrire…
scene-next = Une nouvelle scène
scene-an-empty-one = Une scène vide
status-focus = Mode concentration
status-focus-hint = Mode concentration (Ctrl+Maj+F)
writing-settings = Réglages d’écriture
count-whole-book = Compter tout le livre
count-section = Compter la section en cours d’écriture
digit-group = { "\u202F" }
words =
    { $count ->
        [one] { $shown } mot
       *[other] { $shown } mots
    }
words-not-in-the-book = { $words } hors du livre
words-in-the-book = { $words } dans le livre
words-in-section = { $words } dans « { $section } »
words-in-this-section = { $words } dans cette section
saving = Enregistrement…
all-saved = Tout est enregistré
setting-spelling = Vérifier l’orthographe
setting-spelling-hint = Souligne les mots que le dictionnaire du navigateur ne connaît pas.
setting-justified = Texte justifié
setting-justified-hint = Aligne les lignes des deux côtés comme une page imprimée, en coupant les mots dans la langue du livre.
setting-typewriter = Défilement machine à écrire
setting-typewriter-hint = Garde la ligne que vous écrivez au milieu de la fenêtre.
settings-close = Fermer
contents = Table des matières
this-scene = Cette scène
where-you-are = Où vous êtes
quiet-rearranged = Quelqu’un d’autre a réorganisé le livre autour de « { $section } ».
quiet-removed = Quelqu’un d’autre a supprimé « { $section } ».
quiet-overtaken = Quelqu’un d’autre a modifié le livre entre-temps, cela n’a donc pas pu être fait. Le livre est affiché tel qu’il est maintenant.
going-back = Revenir là où vous en étiez

move-scene = Déplacer la scène
drag-to-move = Faire glisser pour déplacer
scene-actions = Actions de la scène
menu-rename = Renommer
menu-take-out = Retirer du livre
menu-bring-in = Mettre dans le livre…
menu-delete-scene = Supprimer la scène
section-actions = Actions de la section
menu-nest = Imbriquer sous la section précédente
menu-lift = Remonter d’un niveau
menu-remove-section = Supprimer la section
add-here = Ajouter ici
menu-new-scene = Nouvelle scène
menu-new-section = Nouvelle section
menu-new-top-level-section = Nouvelle section de premier niveau
menu-bring-back = Ramener une scène…
scene-title = Titre de la scène
menu-button-title = { $menu } ({ $keys })
keys-tab = Tab
keys-shift-tab = Maj+Tab
keys-shift-f10 = Maj+F10
cancel = Annuler
asked-keep-writing = Continuer d’écrire
answer-delete = Supprimer
answer-remove = Retirer
answer-show-it = La montrer
answer-discard = Abandonner
answer-put-at-end = La mettre à la fin du livre
answer-take-out = Retirer du livre
answer-delete-for-good = Supprimer définitivement
asked-delete-scene = Supprimer cette scène ?
asked-delete-scene-told-unnamed = Cette scène et tout ce qui y est écrit disparaîtront pour de bon. La suppression est irréversible.
asked-delete-scene-told = « { $scene } » et tout ce qui y est écrit disparaîtront pour de bon. La suppression est irréversible.
removed-meanwhile = Supprimé entre-temps
removed-meanwhile-told-unnamed = Quelqu’un d’autre a supprimé « { $section } ». Votre scène est maintenant hors du livre, parmi les scènes qui n’y sont pas.
removed-meanwhile-told = Quelqu’un d’autre a supprimé « { $section } ». Votre scène « { $scene } » est maintenant hors du livre, parmi les scènes qui n’y sont pas.
place-gone = Sa place a disparu
place-gone-scene = Quelqu’un d’autre a supprimé l’endroit où allait votre nouvelle scène. Elle commence par « { $begins } ».
place-gone-section = Quelqu’un d’autre a supprimé ce après quoi votre nouvelle section « { $section } » devait venir.
renamed-meanwhile-heading-told = Pendant que vous modifiiez ce titre, quelqu’un d’autre l’a renommé en « { $theirs } ». Le vôtre est « { $mine } ».
asked-remove-section = Supprimer cette section ?
asked-remove-section-told = Ses scènes quittent le livre sans être supprimées : elles attendent parmi les scènes hors du livre. Les sections qu’elle contient remontent à sa place.
asked-remove-chosen = Retirer ce qui est choisi ?
chosen-scenes =
    { $scenes ->
        [one] { $scenes } scène est choisie.
       *[other] { $scenes } scènes sont choisies.
    }
chosen-sections =
    { $sections ->
        [one] { $sections } section est choisie.
       *[other] { $sections } sections sont choisies.
    }
chosen-both =
    { $scenes ->
        [one] { $scenes } scène
       *[other] { $scenes } scènes
    } et { $sections ->
        [one] { $sections } section
       *[other] { $sections } sections
    } sont choisies.
chosen-whole = Chaque section part avec les sections qu’elle contient et toutes leurs scènes.
chosen-ways-book = Retirées du livre, les scènes attendent parmi celles qui n’y sont pas. Supprimées définitivement, elles et tout ce qui y est écrit disparaissent, et c’est irréversible.
chosen-ways-loose = Supprimées définitivement, elles et tout ce qui y est écrit disparaissent, et c’est irréversible.
placing = Mettre dans le livre
placing-told-unnamed = La scène va à la fin de la section que vous choisissez.
placing-told = « { $scene } » va à la fin de la section que vous choisissez.
placing-sections = Sections du livre
bring-back = Ramener une scène
looking = Recherche…
bring-back-none = Toutes les scènes sont déjà dans le livre.
bring-back-told = Ces scènes ne sont pas dans le livre. Celle que vous choisissez entre là où vous avez cliqué.
bring-back-find = Chercher une scène
bring-back-no-match = Aucune scène ne correspond.

contents-unfold-all = Tout déplier
contents-fold-all = Tout replier
contents-unfold = Déplier { $section }
this-section = Cette section
inspector-nothing-yet = Placez le curseur dans une scène ou un titre pour le voir ici.
inspector-section-removed = Cette section a été supprimée.
pinned-ideas = Idées notées
pinned-none = Aucune idée notée ici.
pin-an-idea = Noter une idée
section-unpin = Retirer { $idea }
drawn-from = S’inspire de
drawn-from-none = Aucune idée liée pour l’instant.
link-an-idea = Lier une idée
unlink-idea = Délier { $idea }
pin-ideas = Noter des idées
ideas-to-pin = Idées à noter
every-idea-pinned = Toutes les idées de ce projet sont déjà notées dans le plan.
pin-ideas-confirm =
    { $count ->
        [0] Noter
        [1] Noter
       *[other] Noter { $count }
    }
link-ideas = Lier des idées
ideas-to-link = Idées à lier
every-idea-linked = Toutes les idées de ce projet sont déjà liées.
link-ideas-confirm =
    { $count ->
        [0] Lier
        [1] Lier
       *[other] Lier { $count }
    }
find-an-idea = Chercher une idée
no-idea-matches = Aucune idée ne correspond.

refusals = Modifications qui n’ont pas pu être appliquées
refusals-told = Elles ont été enregistrées, mais ne sont pas arrivées partout où elles le devaient. Réessayez une fois réglé ce qui les a arrêtées, ou confirmez-les pour faire taire l’alarme.
refusals-none = Il n’y a plus rien à voir.
refusals-list = Modifications refusées
close = Fermer
refusal-wordless = Une modification n’est pas arrivée partout où elle le devait.
refusal-given-up = Abandonnée le { $when }
refusal-details = Détails
refusal-listener = Destinataire
refusal-message = Message
refusal-attempts = Tentatives
refusal-reason = Raison
refusal-retry = Réessayer
refusal-acknowledge = Confirmer
alarm-note = Certaines modifications n’ont pas pu être appliquées.
alarm-show = Afficher
missing = Rien n’est tissé à cette adresse.
missing-back = Retour à vos projets

moment =
    { $day } { $month ->
        [1] janv.
        [2] févr.
        [3] mars
        [4] avr.
        [5] mai
        [6] juin
        [7] juil.
        [8] août
        [9] sept.
        [10] oct.
        [11] nov.
       *[12] déc.
    } { $year } à { $hour }:{ $minute }

sign-in-heading = Se connecter
sign-in-email = Adresse e-mail
sign-in-password = Mot de passe
sign-in-confirm = Se connecter
sign-out = Se déconnecter
refused-email-malformed = Ce n’est pas une adresse e-mail comme nom@exemple.com.
refused-email-too-long = Une adresse e-mail peut compter au plus { $max } caractères ; celle-ci en a { $actual }.
refused-account-name-blank = Un compte a besoin d’un nom.
refused-account-name-control-character = Un nom ne peut pas contenir de caractères de contrôle.
refused-account-name-too-long = Un nom peut compter au plus { $max } caractères ; celui-ci en a { $actual }.
refused-password-too-short = Un mot de passe doit compter au moins { $min } caractères ; celui-ci en a { $actual }.
refused-password-too-long = Un mot de passe peut compter au plus { $max } caractères ; celui-ci en a { $actual }.
refused-verification-code-wrong = Ce n’est pas le code que nous avons envoyé.
refused-verification-code-expired = Ce code a expiré. Veuillez en demander un nouveau.
refused-verification-code-exhausted = Ce code a été essayé trop souvent. Veuillez en demander un nouveau.
refused-sign-in-refused = Cette adresse e-mail et ce mot de passe ne permettent pas de se connecter.
refused-address-not-verified = Cette adresse e-mail n’est pas encore confirmée. Veuillez saisir le code que nous y avons envoyé.
refused-not-signed-in = Veuillez d’abord vous connecter.
sign-up-heading = Créer un compte
sign-up-name = Votre nom
sign-up-password-hint = Au moins 12 caractères.
sign-up-confirm = Créer un compte
sign-up-instead = Pas encore de compte ? Créez-en un
sign-in-instead = Déjà un compte ? Connectez-vous
set-up-instead = Configurer cette instance de Weaveling
set-up-heading = Configurer cette instance de Weaveling
set-up-hint = Personne n’opère encore cette instance de Weaveling. Créez un compte avec le code du journal du serveur ; dès votre adresse confirmée, vous en serez l’opérateur.
set-up-code = Code du journal du serveur
invitation-accept-heading = Vous avez reçu une invitation
invitation-accept-hint = Choisissez votre nom et un mot de passe. Votre compte utilisera l’adresse e-mail à laquelle l’invitation a été envoyée.
invitation-signed-in = Vous êtes déjà connecté en tant que { $email }. Une invitation crée un nouveau compte : pour l’accepter, déconnectez-vous et rouvrez le lien.
code-heading = Confirmez votre adresse
code-sent = Nous avons envoyé un code à { $email }.
code-field = Code
code-confirm = Confirmer
code-again = Envoyer un nouveau code
code-sent-again = Un nouveau code est en route.
code-confirmed = Votre adresse est confirmée. Vous pouvez maintenant vous connecter.
code-enter = Saisir le code
unreachable-heading = Le serveur est injoignable
try-again = Réessayer
account-menu = Compte : { $name }
reset-instead = Mot de passe oublié ?
reset-heading = Choisir un nouveau mot de passe
reset-ask = Envoyer un code
reset-password = Nouveau mot de passe
reset-confirm = Enregistrer le nouveau mot de passe
reset-done = Votre nouveau mot de passe est enregistré. Vous pouvez maintenant vous connecter avec.
back-to-sign-in = Retour à la connexion
refused-too-many-tries =
    { $minutes ->
        [one] Trop de tentatives. Veuillez patienter une minute avant de réessayer.
       *[other] Trop de tentatives. Veuillez patienter { $minutes } minutes avant de réessayer.
    }
refused-operator-code-wrong = Ce n'est pas le code que le serveur a écrit dans le journal.
account-menu-operator = Gérer l'instance
operator-heading = Gérer l'instance
operator-you-are = Vous êtes l'opérateur de cette instance de Weaveling.
operator-become-heading = Devenir l'opérateur
operator-become-hint = Saisissez le code que le serveur a écrit dans son journal au démarrage.
operator-code = Code
operator-become-confirm = Devenir l'opérateur
operator-no-accounts = Cette instance de Weaveling ne garde aucun compte, elle n'a donc pas d'opérateur.
refused-account-locked = Ce compte est verrouillé.
refused-not-an-operator = Seul l'opérateur de cette instance de Weaveling peut faire cela.
refused-operator-cannot-be-locked = L'opérateur ne peut pas être verrouillé.
refused-lock-reason-blank = Verrouiller un compte demande une raison.
refused-lock-reason-control-character = Une raison ne peut pas contenir de caractères de contrôle.
refused-lock-reason-too-long = Une raison peut compter au plus { $max } caractères ; celle-ci en a { $actual }.
refused-account-id-invalid = Ce n'est pas l'identifiant d'un compte.
refused-account-not-found = Ce compte n'existe pas.
instance-sections = Sections
instance-accounts-heading = Comptes
instance-accounts-more = Afficher plus
instance-account-signed-up = Inscrit le { $when }
instance-account-unverified = Adresse non confirmée
instance-invitations-heading = Invitations
instance-invite-email = Adresse e-mail
instance-invite-confirm = Inviter
instance-invitation-withdraw = Retirer
instance-invitations-none = Personne n’est invité pour le moment.
instance-invitations-more = Afficher plus d’invitations
instance-invitation-issued = Invité le { $when }
instance-invitation-open = Ouverte jusqu’au { $when }
instance-invitation-expired = Expirée le { $when }
instance-invitation-declined-address-has-account = Inutile : cette adresse e-mail a déjà un compte
instance-account-operator = Opère l’instance
instance-account-locked = Verrouillé depuis le { $when }
instance-account-lock = Verrouiller…
instance-account-unlock = Déverrouiller
instance-lock-dialog = Verrouiller { $name }
instance-lock-hint = { $name } est déconnecté aussitôt et ne peut plus se connecter tant que le compte reste verrouillé. La raison est gardée avec le verrou et affichée ici.
instance-lock-reason = Raison
instance-lock-cancel = Annuler
instance-lock-confirm = Verrouiller
refused-last-operator = Le dernier opérateur garde le rôle, sinon personne ne pourrait gérer cette instance de Weaveling.
refused-operator-address-not-verified = Seul un compte dont l’adresse est confirmée peut devenir opérateur.
refused-locked-cannot-become-operator = Un compte verrouillé ne peut pas devenir opérateur.
refused-signing-up-by-invitation-only = Sur cette instance de Weaveling, on ne peut créer un compte que sur invitation.
refused-invitation-invalid = Le lien de cette invitation ne permet de créer aucun compte, ou plus maintenant.
refused-invitation-expired = Cette invitation a expiré. Veuillez en demander une nouvelle.
refused-email-address-has-account = Cette adresse e-mail a déjà un compte. Veuillez plutôt vous connecter.
instance-account-appoint = Nommer opérateur
instance-account-dismiss = Retirer le rôle d’opérateur
instance-appoint-dialog = Nommer { $name } opérateur
instance-appoint-hint = { $name } pourra alors gérer cette instance de Weaveling comme vous : lister les comptes, les verrouiller et déverrouiller, et nommer ou retirer des opérateurs.
instance-appoint-confirm = Nommer opérateur
instance-dismiss-dialog = Retirer le rôle d’opérateur à { $name }
instance-dismiss-hint = { $name } ne pourra plus gérer cette instance de Weaveling.
instance-dismiss-yourself-hint = Vous ne pourrez plus gérer cette instance de Weaveling, et seul un autre opérateur pourra vous rendre le rôle.
instance-dismiss-confirm = Retirer le rôle
instance-role-cancel = Annuler
refused-origin-missing = Le lien d’une invitation est formé à partir de la page d’où elle est envoyée, et cette requête n’en nommait aucune.
refused-invitation-id-invalid = Ce n’est pas l’identifiant d’une invitation.
refused-invitation-not-found = Cette invitation n’existe pas.
refused-invitation-already-accepted = Cette invitation a déjà été acceptée.
