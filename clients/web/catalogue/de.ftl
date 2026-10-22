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
theme-chosen = Farbschema: { $theme }

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

book = Das Buch
writing-sides = Seiten des Werks
writing-side-book = Das Buch
writing-side-loose = Nicht im Buch
writing-every-scene-placed = Jede Szene steht im Buch.
heading-untitled = Abschnitt ohne Titel
heading-first = Der erste Abschnitt deines Buchs
heading-next = Ein neuer Abschnitt
scene-invitation = Eine leere Szene. Fang an zu schreiben…
scene-next = Eine neue Szene
scene-an-empty-one = Eine leere Szene
status-focus = Fokusmodus
status-focus-hint = Fokusmodus (Strg+Umschalt+F)
writing-settings = Schreibeinstellungen
count-whole-book = Das ganze Buch zählen
count-section = Den Abschnitt zählen, in dem du schreibst
digit-group = { "." }
words =
    { $count ->
        [one] { $shown } Wort
       *[other] { $shown } Wörter
    }
words-not-in-the-book = { $words } nicht im Buch
words-in-the-book = { $words } im Buch
words-in-section = { $words } in „{ $section }“
words-in-this-section = { $words } in diesem Abschnitt
saving = Wird gespeichert…
all-saved = Alles gespeichert
setting-spelling = Rechtschreibung prüfen
setting-spelling-hint = Unterstreicht Wörter, die das Wörterbuch des Browsers nicht kennt.
setting-justified = Blocksatz
setting-justified-hint = Setzt die Zeilen auf beiden Seiten bündig wie eine gedruckte Seite und trennt Wörter in der Sprache des Buchs.
setting-typewriter = Schreibmaschinenmodus
setting-typewriter-hint = Hält die Zeile, in der du schreibst, in der Mitte des Fensters.
settings-close = Schließen
contents = Inhalt
this-scene = Diese Szene
where-you-are = Wo du bist
quiet-rearranged = Jemand anderes hat das Buch rund um „{ $section }“ umgestellt.
quiet-removed = Jemand anderes hat „{ $section }“ entfernt.
quiet-overtaken = Jemand anderes hat das Buch inzwischen geändert, deshalb ging das nicht. Es zeigt das Buch, wie es jetzt ist.
going-back = Zurück zu der Stelle, an der du aufgehört hast

move-scene = Szene verschieben
drag-to-move = Zum Verschieben ziehen
scene-actions = Szenenaktionen
menu-rename = Umbenennen
menu-take-out = Aus dem Buch nehmen
menu-bring-in = Ins Buch holen…
menu-delete-scene = Szene löschen
section-actions = Abschnittsaktionen
menu-nest = Unter den vorigen Abschnitt einrücken
menu-lift = Eine Ebene herausrücken
menu-remove-section = Abschnitt entfernen
add-here = Hier einfügen
menu-new-scene = Neue Szene
menu-new-section = Neuer Abschnitt
menu-new-top-level-section = Neuer Abschnitt auf oberster Ebene
menu-bring-back = Szene zurückholen…
scene-title = Szenentitel
menu-button-title = { $menu } ({ $keys })
keys-tab = Tab
keys-shift-tab = Umschalt+Tab
keys-shift-f10 = Umschalt+F10
cancel = Abbrechen
asked-keep-writing = Weiterschreiben
answer-delete = Löschen
answer-remove = Entfernen
answer-show-it = Zeigen
answer-discard = Verwerfen
answer-put-at-end = Ans Ende des Buchs stellen
answer-take-out = Aus dem Buch nehmen
answer-delete-for-good = Endgültig löschen
asked-delete-scene = Diese Szene löschen?
asked-delete-scene-told-unnamed = Diese Szene und alles, was darin steht, ist dann für immer weg. Löschen lässt sich nicht rückgängig machen.
asked-delete-scene-told = „{ $scene }“ und alles, was darin steht, ist dann für immer weg. Löschen lässt sich nicht rückgängig machen.
removed-meanwhile = Inzwischen entfernt
removed-meanwhile-told-unnamed = Jemand anderes hat „{ $section }“ entfernt. Deine Szene steht jetzt außerhalb des Buchs, bei den Szenen, die nicht darin stehen.
removed-meanwhile-told = Jemand anderes hat „{ $section }“ entfernt. Deine Szene „{ $scene }“ steht jetzt außerhalb des Buchs, bei den Szenen, die nicht darin stehen.
place-gone = Ihr Platz ist weg
place-gone-scene = Jemand anderes hat entfernt, wohin deine neue Szene kommen sollte. Sie beginnt mit „{ $begins }“.
place-gone-section = Jemand anderes hat entfernt, wonach dein neuer Abschnitt „{ $section }“ kommen sollte.
renamed-meanwhile-heading-told = Während du diese Überschrift bearbeitet hast, hat jemand anderes sie in „{ $theirs }“ umbenannt. Deine lautet „{ $mine }“.
asked-remove-section = Diesen Abschnitt entfernen?
asked-remove-section-told = Seine Szenen verlassen das Buch, werden aber nicht gelöscht: Sie warten bei den Szenen, die nicht im Buch stehen. Die Abschnitte darin rücken an seine Stelle.
asked-remove-chosen = Das Gewählte entfernen?
chosen-scenes =
    { $scenes ->
        [one] { $scenes } Szene ist gewählt.
       *[other] { $scenes } Szenen sind gewählt.
    }
chosen-sections =
    { $sections ->
        [one] { $sections } Abschnitt ist gewählt.
       *[other] { $sections } Abschnitte sind gewählt.
    }
chosen-both =
    { $scenes ->
        [one] { $scenes } Szene
       *[other] { $scenes } Szenen
    } und { $sections ->
        [one] { $sections } Abschnitt
       *[other] { $sections } Abschnitte
    } sind gewählt.
chosen-whole = Jeder Abschnitt geht mit den Abschnitten darin und all ihren Szenen.
chosen-ways-book = Aus dem Buch genommen, warten die Szenen bei denen, die nicht darin stehen. Endgültig gelöscht, sind sie und alles, was darin steht, weg, und das lässt sich nicht rückgängig machen.
chosen-ways-loose = Endgültig gelöscht, sind sie und alles, was darin steht, weg, und das lässt sich nicht rückgängig machen.
placing = Ins Buch holen
placing-told-unnamed = Die Szene kommt ans Ende des Abschnitts, den du wählst.
placing-told = „{ $scene }“ kommt ans Ende des Abschnitts, den du wählst.
placing-sections = Abschnitte des Buchs
bring-back = Szene zurückholen
looking = Wird gesucht…
bring-back-none = Jede Szene steht schon im Buch.
bring-back-told = Diese Szenen stehen nicht im Buch. Die gewählte kommt dorthin, wo du geklickt hast.
bring-back-find = Szene suchen
bring-back-no-match = Keine Szene passt.

contents-unfold-all = Alles aufklappen
contents-fold-all = Alles einklappen
contents-unfold = { $section } aufklappen
this-section = Dieser Abschnitt
inspector-nothing-yet = Setz den Cursor in eine Szene oder eine Überschrift, um sie hier zu sehen.
inspector-section-removed = Dieser Abschnitt wurde entfernt.
pinned-ideas = Vermerkte Ideen
pinned-none = Hier sind keine Ideen vermerkt.
pin-an-idea = Idee vermerken
section-unpin = { $idea } lösen
drawn-from = Schöpft aus
drawn-from-none = Noch keine Ideen verknüpft.
link-an-idea = Idee verknüpfen
unlink-idea = Verknüpfung mit { $idea } lösen
pin-ideas = Ideen vermerken
ideas-to-pin = Ideen zum Vermerken
every-idea-pinned = Jede Idee dieses Projekts ist in der Gliederung schon vermerkt.
pin-ideas-confirm =
    { $count ->
        [0] Vermerken
        [1] Vermerken
       *[other] { $count } vermerken
    }
link-ideas = Ideen verknüpfen
ideas-to-link = Ideen zum Verknüpfen
every-idea-linked = Jede Idee dieses Projekts ist schon verknüpft.
link-ideas-confirm =
    { $count ->
        [0] Verknüpfen
        [1] Verknüpfen
       *[other] { $count } verknüpfen
    }
find-an-idea = Idee suchen
no-idea-matches = Keine Idee passt.

refusals = Änderungen, die nicht übernommen werden konnten
refusals-told = Sie wurden gespeichert, kamen aber nicht überall an, wo sie hin sollten. Versuch es noch einmal, sobald behoben ist, was sie aufgehalten hat, oder bestätige sie, damit der Alarm verstummt.
refusals-none = Hier gibt es nichts mehr zu sehen.
refusals-list = Abgewiesene Änderungen
close = Schließen
refusal-wordless = Eine Änderung kam nicht überall an, wo sie hin sollte.
refusal-given-up = Aufgegeben am { $when }
refusal-details = Details
refusal-listener = Empfänger
refusal-message = Nachricht
refusal-attempts = Versuche
refusal-reason = Grund
refusal-retry = Noch einmal versuchen
refusal-acknowledge = Bestätigen
alarm-note = Einige Änderungen konnten nicht übernommen werden.
alarm-show = Anzeigen
missing = An dieser Adresse ist nichts gewoben.
missing-back = Zurück zu deinen Projekten

moment =
    { $day }. { $month ->
        [1] Jan.
        [2] Feb.
        [3] März
        [4] Apr.
        [5] Mai
        [6] Juni
        [7] Juli
        [8] Aug.
        [9] Sept.
        [10] Okt.
        [11] Nov.
       *[12] Dez.
    } { $year }, { $hour }:{ $minute }

sign-in-heading = Anmelden
sign-in-email = E-Mail-Adresse
sign-in-password = Passwort
sign-in-confirm = Anmelden
sign-out = Abmelden
refused-email-malformed = Das ist keine E-Mail-Adresse wie name@example.com.
refused-email-too-long = Eine E-Mail-Adresse darf höchstens { $max } Zeichen lang sein, diese hat { $actual }.
refused-account-name-blank = Ein Konto braucht einen Namen.
refused-account-name-control-character = Ein Name darf keine Steuerzeichen enthalten.
refused-account-name-too-long = Ein Name darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-password-too-short = Ein Passwort braucht mindestens { $min } Zeichen, dieses hat { $actual }.
refused-password-too-long = Ein Passwort darf höchstens { $max } Zeichen lang sein, dieses hat { $actual }.
refused-verification-code-wrong = Das ist nicht der Code, den wir geschickt haben.
refused-verification-code-expired = Dieser Code ist abgelaufen. Fordere einen neuen an.
refused-verification-code-exhausted = Dieser Code wurde zu oft versucht. Fordere einen neuen an.
refused-sign-in-refused = Mit dieser E-Mail-Adresse und diesem Passwort kannst du dich nicht anmelden.
refused-address-not-verified = Diese E-Mail-Adresse ist noch nicht bestätigt. Gib den Code ein, den wir dorthin geschickt haben.
refused-not-signed-in = Bitte melde dich zuerst an.
sign-up-heading = Registrieren
sign-up-name = Dein Name
sign-up-password-hint = Mindestens 12 Zeichen.
sign-up-confirm = Registrieren
sign-up-instead = Noch kein Konto? Registrieren
sign-in-instead = Schon ein Konto? Anmelden
code-heading = Bestätige deine Adresse
code-sent = Wir haben einen Code an { $email } geschickt.
code-field = Code
code-confirm = Bestätigen
code-again = Neuen Code schicken
code-sent-again = Ein neuer Code ist unterwegs.
code-confirmed = Deine Adresse ist bestätigt. Du kannst dich jetzt anmelden.
code-enter = Code eingeben
unreachable-heading = Der Server ist nicht erreichbar
try-again = Noch einmal versuchen
account-menu = Konto: { $name }
