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

language = Langue
language-machine-translated = { $language } (traduction automatique)

workspace-heading = Vos projets
workspace-tagline = Confiez-nous vos petites idées d’histoires, si fragiles, et nous vous aiderons à les tisser en une véritable épopée.
workspace-loading = Chargement…
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
