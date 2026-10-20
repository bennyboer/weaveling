masthead-all-projects = Alle Projekte
masthead-views = Ansichten
view-write = Schreiben
view-board = Pinnwand
view-outline = Gliederung
view-ideas = Ideen
status-bar = Status

theme = Farbschema
theme-light = Hell
theme-system = Wie das System
theme-dark = Dunkel

language = Sprache
language-machine-translated = { $language } (maschinell übersetzt)

workspace-heading = Deine Projekte
workspace-tagline = Bring uns deine winzigen, zerbrechlichen Ideen für Geschichten, und wir helfen dir, sie zu einem großen Epos zu verweben.
loading = Wird geladen…
workspace-empty = Noch keine Projekte. Jedes Epos beginnt als kleines Durcheinander.

new-project-working-title = Ein Arbeitstitel…
new-project-create = Anlegen
new-project-dialog = Neues Projekt
new-project-confirm = Projekt anlegen

project-more-actions = Weitere Aktionen
project-rename = Umbenennen
project-delete = Löschen
project-rename-save = Speichern
project-rename-cancel = Abbrechen

delete-project-dialog = Dieses Projekt löschen?
delete-project-fate = „{ $name }“ ist dann für immer weg. Löschen lässt sich nicht rückgängig machen.
delete-project-cancel = Abbrechen
delete-project-confirm = Löschen

new-project-cancel = Abbrechen

details-title = Titel
details-book-language = Sprache des Buchs
details-book-language-unset = Nicht festgelegt
details-book-language-hint = Die Sprache, in der das Buch geschrieben ist, überall dort genutzt, wo der Text von ihr abhängt.
details-quotation = Anführungszeichen
details-quotation-as-the-language-has-it = Wie in der Sprache üblich, { $marks }
details-quotation-hint = Die Zeichen, die um das Gesagte gesetzt werden, sobald " getippt wird.
details-description = Beschreibung
details-description-placeholder = Worum es im Buch geht, in ein paar Zeilen. Optional.

book-language-de = Deutsch
book-language-en = Englisch (English)
book-language-fr = Französisch (Français)
book-language-es = Spanisch (Español)
book-language-it = Italienisch (Italiano)
book-language-nl = Niederländisch (Nederlands)
book-language-pt = Portugiesisch (Português)
book-language-pl = Polnisch (Polski)
book-language-sv = Schwedisch (Svenska)
book-language-da = Dänisch (Dansk)
book-language-nb = Norwegisch (Norsk)
book-language-fi = Finnisch (Suomi)
book-language-cs = Tschechisch (Čeština)

project-details = Projektdetails
project-details-cancel = Abbrechen
project-details-save = Speichern
project-details-changing-the-marks = Wechseln die Anführungszeichen von { $from } zu { $to }, werden sie in jeder Szene des Buchs neu gesetzt. In einem langen Buch dauert das eine Weile.
project-details-back = Zurück
project-details-change-the-marks = Zeichen wechseln

restyling = Die Anführungszeichen werden neu gesetzt
restyling-waiting = Die Anführungszeichen werden neu gesetzt…
restyling-running = Die Anführungszeichen werden neu gesetzt: { $done } von { $total } { $total ->
        [one] Szene
       *[other] Szenen
    }.
restyling-finished = Jede Szene setzt jetzt { $marks }.
restyling-close = Schließen

problem-not-found =
    { $subject ->
        [appearances] Diese Auftritte gibt es nicht mehr.
        [board] Diese Pinnwand gibt es nicht mehr.
        [idea] Diese Idee gibt es nicht mehr.
        [outline] Diese Gliederung gibt es nicht mehr.
        [project] Dieses Projekt gibt es nicht mehr.
        [refusal] Diese Meldung gibt es nicht mehr.
        [scene] Diese Szene gibt es nicht mehr.
        [word-counts] Diese Wortzahlen gibt es nicht mehr.
       *[other] Das gibt es nicht mehr.
    }
problem-outdated =
    { $subject ->
        [appearances] Diese Auftritte wurden inzwischen anderswo geändert. Schließ das hier und öffne es neu, um den aktuellen Stand zu sehen.
        [board] Diese Pinnwand wurde inzwischen anderswo geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [idea] Diese Idee wurde inzwischen anderswo geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [outline] Diese Gliederung wurde inzwischen anderswo geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [project] Dieses Projekt wurde inzwischen anderswo geändert. Schließ das hier und öffne es neu, um den aktuellen Stand zu sehen.
        [refusal] Diese Meldung wurde inzwischen anderswo geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [scene] Diese Szene wurde inzwischen anderswo geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [word-counts] Diese Wortzahlen wurden inzwischen anderswo geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
       *[other] Das wurde inzwischen anderswo geändert. Schließ das hier und öffne es neu, um den aktuellen Stand zu sehen.
    }
problem-overtaken = Jemand anderes hat das inzwischen geändert, deshalb ging es nicht.
problem-offline = Der Server ist gerade nicht erreichbar. Bitte versuch es gleich noch einmal.
problem-unexpected = Etwas Unerwartetes ist schiefgegangen. Bitte versuch es noch einmal.
problem-dismiss = Schließen

refused-project-id-invalid = Das ist keine gültige Projekt-ID.
refused-project-name-blank = Ein Projekt braucht einen Titel.
refused-project-name-control-character = Ein Titel darf keine Steuerzeichen enthalten.
refused-project-name-too-long = Ein Titel darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-book-language-not-a-tag = „{ $tag }“ ist kein Sprachkürzel wie „de“, „en“ oder „en-GB“.
refused-quotation-marks-unknown = „{ $name }“ sind keine Anführungszeichen, die Weaveling kennt.
refused-project-description-control-character = Eine Beschreibung darf außer Zeilenumbrüchen keine Steuerzeichen enthalten.
refused-project-description-too-long = Eine Beschreibung darf höchstens { $max } Zeichen lang sein, diese hat { $actual }.
refused-project-deleted = Dieses Projekt wurde inzwischen gelöscht.
refused-board-id-invalid = Das ist keine gültige Pinnwand-ID.
refused-idea-already-pinned = Diese Idee hängt schon an der Pinnwand.
refused-idea-not-pinned = Diese Idee hängt nicht an der Pinnwand.
refused-card-shapeless = Eine Karte braucht eine Breite und eine Höhe.
refused-board-discarded = Diese Pinnwand wurde inzwischen verworfen.
refused-idea-id-invalid = Das ist keine gültige Ideen-ID.
refused-idea-title-control-character = Der Titel einer Idee darf keine Steuerzeichen enthalten.
refused-idea-title-too-long = Der Titel einer Idee darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-idea-discarded = Diese Idee wurde inzwischen verworfen.
refused-outline-id-invalid = Das ist keine gültige Gliederungs-ID.
refused-section-title-control-character = Der Titel eines Abschnitts darf keine Steuerzeichen enthalten.
refused-section-title-too-long = Der Titel eines Abschnitts darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-outline-holds-no-such-kind = Eine Gliederung enthält Szenen und Ideen, sonst nichts.
refused-section-not-found = Diesen Abschnitt gibt es in der Gliederung nicht mehr.
refused-section-would-contain-itself = Ein Abschnitt kann nicht in sich selbst verschoben werden.
refused-section-neighbour-not-found = An dieser Stelle steht nichts, hinter das es kommen könnte.
refused-not-in-the-outline = Das steht nicht in der Gliederung.
refused-outline-discarded = Diese Gliederung wurde inzwischen verworfen.
refused-scene-id-invalid = Das ist keine gültige Szenen-ID.
refused-scene-title-control-character = Der Titel einer Szene darf keine Steuerzeichen enthalten.
refused-scene-title-too-long = Der Titel einer Szene darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-quotation-never-restyled = Die Anführungszeichen dieses Buchs wurden nie neu gesetzt.

untitled = Ohne Titel
board = Pinnwand
board-not-on-it = Nicht an der Pinnwand · { $count }
board-waiting = Ideen, die nicht an der Pinnwand hängen
board-every-idea-pinned = Jede Idee hängt an der Pinnwand.
board-how-to-pin = Klick auf eine Idee, um sie dort anzupinnen, wo Platz ist.
board-pin = { $idea } anpinnen
board-actions = Aktionen für { $idea }
board-rename = { $idea } umbenennen
board-open = { $idea } öffnen
board-unpin = { $idea } abnehmen
idea-asked = Was ist die Idee?
board-zoom = Zoom
board-zoom-out = Verkleinern
board-zoom-reset = Zoom zurücksetzen
board-zoom-in = Vergrößern
tray = Ablage
inspector = Inspektor
inspector-idea-name = Name der Idee
inspector-appears-in = Kommt vor in
inspector-appears-nowhere = Noch nicht im Buch — kein Abschnitt vermerkt sie, keine Szene greift auf sie zurück.
board-resize =
    { $side ->
        [top] Größe von { $idea } oben ändern
        [bottom] Größe von { $idea } unten ändern
        [left] Größe von { $idea } links ändern
        [right] Größe von { $idea } rechts ändern
        [top-left] Größe von { $idea } oben links ändern
        [top-right] Größe von { $idea } oben rechts ändern
        [bottom-left] Größe von { $idea } unten links ändern
        [bottom-right] Größe von { $idea } unten rechts ändern
       *[other] Größe von { $idea } ändern
    }
drawer-close =
    { $drawer ->
        [tray] Ablage schließen
        [contents] Inhaltsverzeichnis schließen
       *[other] Schließen
    }
drawer-resize =
    { $drawer ->
        [tray] Breite der Ablage ändern
        [contents] Breite des Inhaltsverzeichnisses ändern
       *[other] Breite ändern
    }

scene-empty = Leer
outline-manuscript = Manuskript
outline-the-manuscript = Das Manuskript
outline-nothing-yet = Noch nichts im Buch. Leg einen Abschnitt an, um zu beginnen.
outline-add-section = Abschnitt anlegen
outline-not-in-the-book = Nicht im Buch · { $count }
outline-scenes = Szenen
outline-ideas = Ideen
outline-fold = { $section } einklappen
outline-section = Abschnitt { $section }
outline-actions = Aktionen für { $section }
outline-move = { $section } verschieben
outline-move-earlier = { $section } nach vorn
outline-move-later = { $section } nach hinten
outline-promote = { $section } eine Ebene höher
outline-demote = { $section } eine Ebene tiefer
outline-write-in = In { $section } schreiben
outline-remove = { $section } entfernen
outline-take-out = { $name } aus dem Buch nehmen
outline-scenes-waiting = Szenen, die nicht im Buch stehen
outline-every-scene-placed = Jede Szene hat ihren Platz im Buch.
outline-ideas-waiting = Ideen, die nicht im Buch stehen
outline-place = { $name } einordnen
outline-how-to-place = Zieh sie auf einen Abschnitt, oder klick sie an und dann dorthin, wo sie hingehört. Eine Szene wird zum Buch, eine Idee zur Notiz daneben.
renamed-meanwhile = Inzwischen umbenannt
renamed-meanwhile-told = Während du diesen Titel bearbeitet hast, hat jemand anderes ihn in „{ $theirs }“ umbenannt. Deiner lautet „{ $mine }“.
renamed-meanwhile-theirs = Ihren übernehmen
renamed-meanwhile-mine = Meinen behalten

ideas = Ideen
ideas-capture = Festhalten
ideas-empty = Noch keine Ideen. Schieß eine Idee rein und schau, wohin sie führt.
idea-opening = Wird geöffnet…
