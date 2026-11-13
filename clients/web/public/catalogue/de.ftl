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
masthead-more = Mehr
masthead-this-project = Dieses Projekt
language-machine-translated = { $language } (maschinell übersetzt)

workspace-heading = Deine Projekte
workspace-tagline = Bring uns deine winzigen, zerbrechlichen Ideen für Geschichten, und wir helfen dir, sie zu einem großen Epos zu verweben.
loading = Wird geladen…
workspace-empty = Noch keine Projekte. Jedes Epos beginnt als kleines Durcheinander.
workspace-trash-link = Papierkorb, { $count } { $count ->
        [one] Projekt
       *[other] Projekte
    }

new-project-working-title = Ein Arbeitstitel…
new-project-create = Anlegen
new-project-dialog = Neues Projekt
new-project-confirm = Projekt anlegen
new-project-setting-up = Dein Projekt wird eingerichtet…

project-more-actions = Weitere Aktionen
project-rename = Umbenennen
project-trash = In den Papierkorb
project-rename-save = Speichern
project-rename-cancel = Abbrechen

trash-project-dialog = Dieses Projekt in den Papierkorb verschieben?
trash-project-fate = „{ $name }“ lässt sich 30 Tage lang aus dem Papierkorb wiederherstellen. Danach ist es für immer weg.
trash-project-cancel = Abbrechen
trash-project-confirm = In den Papierkorb
trash-heading = Papierkorb
trash-empty = Der Papierkorb ist leer.
trash-kept-until = Wiederherstellbar bis { $moment }
trash-restore = Wiederherstellen

new-project-cancel = Abbrechen

details-title = Titel
details-book-language = Sprache des Buchs
details-book-language-unset = Nicht festgelegt
details-book-language-hint = Die Sprache, in der dein Buch geschrieben ist. Silbentrennung und Anführungszeichen richten sich danach.
details-quotation = Anführungszeichen
details-quotation-as-the-language-has-it = Wie in der Sprache üblich, { $marks }
details-quotation-hint = Die Zeichen, die um wörtliche Rede gesetzt werden, wenn du " tippst.
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
        [account] Dieses Konto gibt es nicht mehr.
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
        [appearances] Diese Auftritte wurden inzwischen von jemand anderem geändert. Schließ das hier und öffne es neu, um den aktuellen Stand zu sehen.
        [board] Diese Pinnwand wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [idea] Diese Idee wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [outline] Diese Gliederung wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [project] Dieses Projekt wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne es neu, um den aktuellen Stand zu sehen.
        [refusal] Diese Meldung wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [scene] Diese Szene wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
        [word-counts] Diese Wortzahlen wurden inzwischen von jemand anderem geändert. Schließ das hier und öffne sie neu, um den aktuellen Stand zu sehen.
       *[other] Das wurde inzwischen von jemand anderem geändert. Schließ das hier und öffne es neu, um den aktuellen Stand zu sehen.
    }
problem-overtaken = Jemand anderes hat das inzwischen geändert, deshalb ging es nicht.
problem-offline = Der Server ist gerade nicht erreichbar. Bitte versuch es gleich noch einmal.
problem-unexpected = Etwas Unerwartetes ist schiefgegangen. Bitte versuch es noch einmal.
problem-dismiss = Schließen

refused-project-id-invalid = Das ist keine gültige Projekt-ID.
refused-project-name-blank = Ein Projekt braucht einen Titel.
refused-project-name-control-character = Ein Titel darf keine Steuerzeichen enthalten.
refused-project-name-too-long = Ein Titel darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-project-not-yet-open = Dein Projekt wurde angelegt, lässt sich aber noch nicht öffnen. Gleich erscheint es in deiner Liste.
refused-book-language-not-a-tag = „{ $tag }“ ist kein Sprachkürzel wie „de“, „en“ oder „en-GB“.
refused-quotation-marks-unknown = Weaveling kennt die Anführungszeichen „{ $name }“ nicht.
refused-project-description-control-character = Eine Beschreibung darf außer Zeilenumbrüchen keine Steuerzeichen enthalten.
refused-project-description-too-long = Eine Beschreibung darf höchstens { $max } Zeichen lang sein, diese hat { $actual }.
refused-project-deleted = Dieses Projekt wurde inzwischen gelöscht.
refused-project-trashed = Dieses Projekt wurde inzwischen in den Papierkorb verschoben.
refused-project-not-trashed = Dieses Projekt liegt nicht im Papierkorb.
refused-not-an-owner = Das darf nur, wem dieses Projekt gehört.
refused-last-owner = Einem Projekt muss immer jemand gehören. Lass es zuerst noch jemand anderem gehören.
refused-not-a-member = Diese Person gehört nicht zu diesem Projekt.
refused-role-unknown = Eine solche Rolle gibt es in einem Projekt nicht.
refused-member-leaving = Dieses Konto wird bald gelöscht, daher lässt sich seine Rolle nicht mehr ändern.
refused-not-an-author = Das darf nur, wer an diesem Projekt mitschreibt.
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
refused-section-neighbour-not-found = Das Element, nach dem es kommen soll, ist nicht mehr da.
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
inspector-appears-nowhere = Noch nicht im Buch. Kein Abschnitt und keine Szene verwendet sie.
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
outline-how-to-place = Zieh sie auf einen Abschnitt, oder klick sie an und dann dorthin, wo sie hin soll. Eine Szene wird Teil des Buchs. Eine Idee wird zur Notiz daneben.
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
path = Pfad
path-home = Startseite
path-invitation = Einladung
path-missing = Nicht gefunden
quiet-rearranged = Jemand anderes hat das Buch rund um „{ $section }“ umgestellt.
quiet-removed = Jemand anderes hat „{ $section }“ entfernt.
quiet-overtaken = Jemand anderes hat das Buch inzwischen geändert, deshalb ging das nicht. Du siehst jetzt den aktuellen Stand.
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
removed-meanwhile-told-unnamed = Jemand anderes hat „{ $section }“ entfernt. Deine Szene findest du jetzt bei den Szenen, die nicht im Buch stehen.
removed-meanwhile-told = Jemand anderes hat „{ $section }“ entfernt. Deine Szene „{ $scene }“ findest du jetzt bei den Szenen, die nicht im Buch stehen.
place-gone = Ihr Platz ist weg
place-gone-scene = Jemand anderes hat die Stelle entfernt, an die deine neue Szene kommen sollte. Sie beginnt mit „{ $begins }“.
place-gone-section = Jemand anderes hat das Element entfernt, nach dem dein neuer Abschnitt „{ $section }“ kommen sollte.
renamed-meanwhile-heading-told = Während du diese Überschrift bearbeitet hast, hat jemand anderes sie in „{ $theirs }“ umbenannt. Deine lautet „{ $mine }“.
asked-remove-section = Diesen Abschnitt entfernen?
asked-remove-section-told = Seine Szenen werden aus dem Buch genommen, aber nicht gelöscht. Du findest sie bei den Szenen, die nicht im Buch stehen. Die Abschnitte darin rücken eine Ebene nach oben.
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
chosen-whole = Jeder Abschnitt wird zusammen mit den Abschnitten darin und all ihren Szenen entfernt.
chosen-ways-book = Nimmst du sie aus dem Buch, kommen die Szenen zu denen, die nicht im Buch stehen. Löschst du sie, sind sie und alles, was darin steht, endgültig weg. Das lässt sich nicht rückgängig machen.
chosen-ways-loose = Löschst du sie, sind sie und alles, was darin steht, endgültig weg. Das lässt sich nicht rückgängig machen.
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
refusals-told = Diese Änderungen wurden gespeichert, aber nicht alle Teile von Weaveling haben sie erhalten. Behebe die Ursache und versuch es noch einmal, oder bestätige sie, um den Alarm abzuschalten.
refusals-none = Hier gibt es nichts mehr zu sehen.
refusals-list = Abgewiesene Änderungen
close = Schließen
refusal-wordless = Eine Änderung ist nicht in allen Teilen von Weaveling angekommen.
refusal-given-up = Aufgegeben am { $when }
refusal-details = Details
refusal-listener = Empfänger
refusal-message = Nachricht
refusal-attempts = Versuche
refusal-reason = Grund
refusal-retry = Noch einmal versuchen
refusal-too-old = Diese Änderung ist älter als 30 Tage und wird nicht noch einmal versucht, weil es das, worum es ging, vielleicht nicht mehr gibt. Bestätige sie, um den Alarm abzuschalten.
refusal-acknowledge = Bestätigen
alarm-note = Einige Änderungen konnten nicht übernommen werden.
alarm-show = Anzeigen
missing = An dieser Adresse ist nichts gewoben.

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

day =
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
    } { $year }

deletion-pending-heading = Dein Konto wird gelöscht
deletion-pending-asked = Du hast die Löschung deines Kontos am { $asked } veranlasst. Am { $until } werden dein Konto und die Projekte, in denen nur du Mitglied bist, endgültig gelöscht.
deletion-pending-nothing-lost = Bis dahin geht nichts verloren. Behältst du dein Konto, bleibst du angemeldet und findest jedes Projekt so vor, wie du es verlassen hast.
deletion-pending-keep = Mein Konto behalten
deletion-pending-let-go = Abmelden und löschen lassen
deletion-pending-by-the-operator = Wer diese Weaveling-Instanz verwaltet, hat die Löschung deines Kontos für den { $until } geplant. Frag nach, wenn du es behalten willst.
deletion-pending-sign-out = Abmelden
your-account-heading = Dein Konto
your-account-security = Wie du dich anmeldest:
your-account-delete-heading = Dieses Konto löschen
your-account-delete-hint = Dein Konto und alles, was dazugehört, werden nach 30 Tagen endgültig gelöscht.
your-account-delete = Dieses Konto löschen …
your-account-delete-question = Dein Konto löschen?
your-account-delete-kept = Du wirst sofort überall abgemeldet. Dein Konto und deine Arbeit bleiben 30 Tage erhalten, bis zum { $until }. Melde dich vorher an, um alles zu behalten. Danach ist es endgültig weg.
your-account-delete-alone = Projekte, in denen nur du Mitglied bist, werden ebenfalls gelöscht.
your-account-delete-password = Dein Passwort, damit wir sicher sind, dass du es bist
your-account-keep = Mein Konto behalten
your-account-delete-confirm = Mein Konto löschen
your-account-hand-over-heading = Übergib diese Projekte zuerst
your-account-hand-over-hint = Dir allein gehören Projekte, in denen auch andere Mitglied sind. Lass jedes davon noch jemand anderem gehören oder lösche es. Dann kannst du dein Konto löschen.
your-account-hand-over-members = Mitglieder
your-account-hand-over-close = Schließen

sign-in-heading = Anmelden
sign-in-email = E-Mail-Adresse
sign-in-password = Passwort
sign-in-confirm = Anmelden
passkey-sign-in = Mit Passkey anmelden
passkey-or-password = oder mit deinem Passwort
passkey-not-touched = Es wurde kein Passkey verwendet. Versuche es noch einmal oder melde dich mit deinem Passwort an.
passkey-failed = Dein Browser konnte nicht nach einem Passkey fragen. Melde dich mit deinem Passwort an.
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
refused-sign-in-refused = Die E-Mail-Adresse oder das Passwort ist falsch.
refused-address-not-verified = Diese E-Mail-Adresse ist noch nicht bestätigt. Gib den Code ein, den wir dorthin geschickt haben.
refused-not-signed-in = Bitte melde dich zuerst an.
sign-up-heading = Registrieren
sign-up-name = Dein Name
sign-up-password-hint = Mindestens 12 Zeichen.
sign-up-confirm = Registrieren
sign-up-instead = Noch kein Konto? Registrieren
sign-in-instead = Schon ein Konto? Anmelden
set-up-instead = Diese Weaveling-Instanz einrichten
set-up-heading = Diese Weaveling-Instanz einrichten
set-up-hint = Diese Weaveling-Instanz wird noch von niemandem verwaltet. Registriere dich mit dem Code aus dem Server-Log. Sobald deine Adresse bestätigt ist, verwaltest du sie.
set-up-code = Code aus dem Server-Log
set-up-done = Diese Weaveling-Instanz ist schon eingerichtet und wird bereits verwaltet. Wer sie verwaltet, kann dich auch zum Verwalter ernennen.
invitation-accept-heading = Du bist eingeladen
invitation-accept-hint = Wähle deinen Namen und ein Passwort. Dein Konto nutzt die E-Mail-Adresse, an die die Einladung ging.
invitation-signed-in = Du bist als { $email } angemeldet. Nimm die Einladung an, um dem Projekt beizutreten.
invitation-accept = Annehmen
invitation-accepted = Einladung angenommen. Das Projekt erscheint gleich in deiner Liste.
invitation-decline = Ablehnen
invitation-declined = Du hast die Einladung abgelehnt.
invitation-decline-instead = Einladung stattdessen ablehnen
invitation-declined-heading = Einladung abgelehnt
invitation-declined-hint = Du hast sie abgelehnt. Die Person, die dich eingeladen hat, sieht das, und der Link funktioniert nicht mehr.
notifications = Benachrichtigungen
notifications-waiting = Benachrichtigungen: { $count } neu
notifications-none = Gerade gibt es nichts Neues.
notifications-invitations = Einladungen
notice-invitation-from = Von { $by }
notice-invitation-unnamed-project = ein Projekt
notice-invitation-join = Beitreten
notice-invitation-decline = Ablehnen
notice-invitation-joined = Du bist { $project } beigetreten. Es erscheint gleich in deiner Liste.
notice-invitation-declined = Du hast die Einladung zu { $project } abgelehnt.
notifications-roles = Rollen
notice-role-unnamed-project = einem Projekt
notice-role-changed-by = { $by } hat deine Rolle in { $project } zu { $role ->
        [owner] Eigentümer:in
        [author] Autor:in
       *[reader] Leser:in
    } geändert.
notice-role-changed = Deine Rolle in { $project } ist jetzt { $role ->
        [owner] Eigentümer:in
        [author] Autor:in
       *[reader] Leser:in
    }.
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
reset-instead = Passwort vergessen?
reset-heading = Neues Passwort festlegen
reset-ask = Code schicken
reset-password = Neues Passwort
reset-confirm = Neues Passwort festlegen
reset-done = Dein neues Passwort ist gesetzt. Du kannst dich jetzt damit anmelden.
back-to-sign-in = Zurück zur Anmeldung
refused-too-many-tries =
    { $minutes ->
        [one] Zu viele Versuche. Bitte warte eine Minute, bevor du es noch einmal versuchst.
       *[other] Zu viele Versuche. Bitte warte { $minutes } Minuten, bevor du es noch einmal versuchst.
    }
refused-operator-code-wrong = Das ist nicht der Code, den der Server ins Log geschrieben hat.
account-menu-operator = Instanz verwalten
path-account = Dein Konto
security-heading = Sicherheit
security-hint = Wie du dich bei dieser Weaveling-Instanz anmeldest.
security-second-step = Zweiter Schritt
security-second-step-hint = Nach deinem Passwort kann Weaveling nach einem zweiten Schritt fragen, sodass ein Passwort allein nicht zur Anmeldung reicht.
security-app = Authenticator-App
security-app-on-since = An seit { $when }
security-app-actions = Aktionen für die Authenticator-App
security-codes = Wiederherstellungscodes
security-codes-left = { $left } von 10 übrig
security-set-up-app = Authenticator-App einrichten
security-password = Dein Passwort, um sicherzugehen, dass du es bist
security-key-named = Sicherheitsschlüssel „{ $name }“
security-key-added = Hinzugefügt { $when }
security-key-actions = Aktionen für { $name }
security-add-key = Sicherheitsschlüssel hinzufügen
key-name = Name
key-name-example = YubiKey blau
key-made-heading = Dein Schlüssel ist erstellt
key-made-hint = Dein Browser hat ihn registriert. Gib ihm einen Namen, an dem du ihn wiedererkennst. Du nutzt ihn als zweiten Schritt nach deinem Passwort.
key-made-dismiss = Nicht hinzufügen
key-made-done = Schlüssel hinzufügen
key-added = Der Schlüssel ist hinzugefügt. Überall sonst wurdest du abgemeldet.
key-added-with-codes = Der Schlüssel ist hinzugefügt, und die Anmeldung fragt jetzt nach einem zweiten Schritt. Überall sonst wurdest du abgemeldet.
key-not-made = Es wurde kein Schlüssel erstellt.
key-held-here = Dieser Schlüssel gehört schon zu deinem Konto.
key-failed = Dein Browser konnte keinen Schlüssel erstellen. Versuche einen anderen Browser oder Schlüssel.
key-rename = Umbenennen
key-rename-heading = „{ $name }“ umbenennen
key-rename-confirm = Speichern
key-remove = Entfernen
key-remove-heading = „{ $name }“ entfernen?
key-remove-hint = Du kannst dich nicht mehr mit diesem Schlüssel anmelden, und überall sonst wirst du abgemeldet.
key-remove-confirm = Entfernen
key-removed = Der Schlüssel ist entfernt. Überall sonst wurdest du abgemeldet.
security-signing-in = Anmeldung
security-passkey-named = Passkey „{ $name }“
security-add-passkey = Passkey hinzufügen
security-welcome = Dein Konto ist bereit. Ein Passkey oder ein zweiter Schritt lässt sich jetzt hinzufügen oder später jederzeit auf dieser Seite.
security-welcome-later = Nicht jetzt, zu meinen Projekten
key-promote = Als Passkey nutzen
key-promote-heading = „{ $name }“ zum Passkey machen
key-promote-hint = Der Schlüssel wird neu als Passkey eingerichtet, damit er allein zur Anmeldung reicht. Dein Browser bittet dich, ihn zu berühren, und um seine PIN oder deinen Fingerabdruck. Danach ersetzt er den Schlüssel, wie er jetzt ist. War er dein einziger zweiter Schritt, wird bei der Anmeldung keiner mehr abgefragt, deine Wiederherstellungscodes funktionieren nicht mehr, und überall sonst wirst du abgemeldet.
key-promote-confirm = Zum Passkey machen
key-promoted = Der Schlüssel ist jetzt ein Passkey und reicht allein zur Anmeldung.
key-demote = Als zweiten Schritt nutzen
key-demote-heading = „{ $name }“ als zweiten Schritt nutzen
key-demote-hint = Der Passkey reicht nicht mehr allein zur Anmeldung. Bei der Anmeldung wirst du zuerst nach deinem Passwort gefragt, dann nach dem Schlüssel. Überall sonst wirst du abgemeldet.
key-demote-confirm = Als zweiten Schritt nutzen
key-demoted = Der Passkey ist jetzt dein zweiter Schritt.
key-demoted-with-codes = Der Passkey ist jetzt dein zweiter Schritt, und bei der Anmeldung wirst du erst nach deinem Passwort, dann nach ihm gefragt. Überall sonst wurdest du abgemeldet.
two-step-unavailable = Diese Weaveling-Instanz unterstützt die Anmeldung in zwei Schritten noch nicht. Bitte den Betreiber, sie einzurichten.
two-step-set-up-anew = Neu einrichten
two-step-set-up-heading = Anmeldung in zwei Schritten einrichten
two-step-set-up-hint = Nach deinem Passwort fragt Weaveling künftig nach einem Code aus einer App auf deinem Telefon.
two-step-scan = 1 · Scanne dies mit deiner Authenticator-App
two-step-qr = QR-Code für deine Authenticator-App
two-step-cannot-scan = Scannen geht nicht? Tippe diesen Schlüssel ein:
two-step-copy-key = Schlüssel kopieren
two-step-code = 2 · Gib den Code ein, den deine App zeigt
two-step-cancel = Abbrechen
two-step-confirm = Bestätigen
two-step-nothing-yet = Es ändert sich nichts, bis dieser Code bestätigt ist.
two-step-on-now = Die Anmeldung in zwei Schritten ist an. Überall sonst wurdest du abgemeldet.
two-step-codes-heading = Bewahre diese Wiederherstellungscodes auf
two-step-codes-hint = Verlierst du dein Telefon, kannst du jeden Code einmal statt des Codes aus der App nutzen. Sie werden nur jetzt angezeigt.
two-step-codes-download = Als Text herunterladen
two-step-codes-copy = Kopieren
two-step-codes-print = Drucken
two-step-codes-kept = Ich habe sie sicher aufbewahrt
two-step-done = Fertig
copy-done = Kopiert
copy-refused = Kopieren ging nicht
second-step-code = Der Code, den deine App für Weaveling zeigt
second-step-confirm = Bestätigen
second-step-key-touch = Berühre deinen Sicherheitsschlüssel.
second-step-key-use = Schlüssel verwenden
second-step-key-not-touched = Es wurde kein Schlüssel berührt. Versuche es noch einmal oder nimm einen anderen Weg.
second-step-key-failed = Dein Browser konnte nicht nach dem Schlüssel fragen. Nimm einen anderen Weg.
second-step-another-way = Einen anderen Weg nutzen
second-step-first-from-now-on = Diesen Weg ab jetzt zuerst nutzen
another-way-heading = Ein anderer Weg
another-way-hint = Wähle, wie du diesmal bestätigst, dass du es bist.
another-way-key = Sicherheitsschlüssel
another-way-key-hint = Berühre deinen Schlüssel
another-way-app = Authenticator-App
another-way-app-hint = Gib den Code ein, den sie zeigt
another-way-recovery = Wiederherstellungscode
another-way-recovery-hint = Jeder Code funktioniert einmal
another-way-asked-first = Wird gerade zuerst abgefragt
security-preferred-way = Zuerst abfragen
security-preferred-key = Einen Sicherheitsschlüssel
security-preferred-key-hint = Den Schlüssel berühren
security-preferred-app = Die Authenticator-App
security-preferred-app-hint = Den Code eingeben, den sie zeigt
security-preferred-last-used = Den, den ich zuletzt genutzt habe
security-preferred-last-used-hint = Der Weg, den du zuletzt genutzt hast, auf jedem Gerät
second-step-recovery-code = Einer deiner Wiederherstellungscodes
second-step-recovery-hint = Jeder Code funktioniert einmal. Groß oder klein, mit oder ohne Bindestrich.
second-step-no-codes = Keine Codes mehr und kein Telefon? Der Betreiber dieser Weaveling-Instanz kann die Anmeldung in zwei Schritten für dich ausschalten.
second-step-not-you = nicht du?
second-step-ran-out = Die Anmeldung hat zu lange gedauert. Bitte melde dich noch einmal an.
recovery-note = Du hast dich mit einem Wiederherstellungscode angemeldet; { $left } sind übrig. Telefon verloren?
recovery-note-last = Das war dein letzter Wiederherstellungscode. Erstelle neue oder richte die Anmeldung in zwei Schritten neu ein, bevor du dich abmeldest.
recovery-note-set-up-anew = Anmeldung in zwei Schritten neu einrichten
two-step-draw-codes = Neue Codes erstellen
two-step-draw-codes-hint = Die neuen Codes ersetzen die alten, die danach nicht mehr funktionieren.
two-step-codes-drawn = Neue Wiederherstellungscodes wurden erstellt. Die alten funktionieren nicht mehr.
two-step-turn-off = Ausschalten
two-step-turn-off-hint = Bei der Anmeldung wird nicht mehr nach einem Code aus deiner App gefragt, und überall sonst wirst du abgemeldet. Ist kein Schlüssel als zweiter Schritt eingerichtet, funktionieren auch deine Wiederherstellungscodes nicht mehr.
two-step-set-up-password-hint = Gib zuerst dein Passwort ein. Dann siehst du den Code, den du in deiner App hinzufügst. Richtest du sie neu ein, ersetzt das die App, die du jetzt nutzt, und deine Wiederherstellungscodes.
security-add-passkey-hint = Gib zuerst dein Passwort ein. Dann fragt dein Browser nach dem Passkey.
security-add-key-hint = Gib zuerst dein Passwort ein. Dann fragt dein Browser nach dem Schlüssel.
security-password-go-on = Weiter
two-step-turned-off = Die Anmeldung in zwei Schritten ist aus. Überall sonst wurdest du abgemeldet.
instance-account-two-step = Anmeldung in zwei Schritten
instance-account-ways-in = Anmeldemethoden
instance-account-actions = Aktionen für { $name }
instance-ways-dialog = Wie sich { $name } anmeldet
instance-ways-password = Passwort
instance-ways-password-hint = Bei der Registrierung gesetzt, durch Zurücksetzen des Passworts geändert
instance-ways-always-kept = lässt sich nicht entfernen
instance-ways-signs-in-alone = reicht allein zur Anmeldung
instance-ways-second-step = zweiter Schritt
instance-ways-remove = Entfernen
instance-ways-remove-named = { $name } entfernen
instance-ways-turn-off = Ausschalten
instance-ways-turn-off-app = App ausschalten
instance-ways-removing = „{ $name }“ entfernen
instance-ways-turning-off = Authenticator-App ausschalten
instance-ways-still-password = { $name } meldet sich weiter mit dem Passwort an und wird überall abgemeldet.
instance-ways-still-second-step = { $name } meldet sich weiter mit dem Passwort und einem zweiten Schritt an und wird überall abgemeldet.
instance-ways-password-alone = { $name } meldet sich nur noch mit dem Passwort an, bis wieder ein zweiter Schritt eingerichtet ist. Die Wiederherstellungscodes gelten nicht mehr, und { $name } wird überall abgemeldet.
instance-ways-make-sure = Stelle vorher außerhalb von Weaveling sicher, dass die Bitte wirklich von dieser Person kommt. Der Verlauf des Kontos zeigt, dass du das getan hast.
instance-ways-keep = Behalten
instance-ways-remove-passkey = Passkey entfernen
instance-ways-remove-key = Schlüssel entfernen
instance-ways-close = Schließen
operator-heading = Instanz verwalten
operator-you-are = Du verwaltest diese Weaveling-Instanz.
operator-become-heading = Verwaltung übernehmen
operator-become-hint = Gib den Code ein, den der Server beim Start ins Log geschrieben hat.
operator-code = Code
operator-become-confirm = Verwaltung übernehmen
operator-no-accounts = Diese Weaveling-Instanz hat keine Konten, also verwaltet sie auch niemand.
refused-account-locked = Dieses Konto ist gesperrt.
refused-not-an-operator = Das darf nur, wer diese Weaveling-Instanz verwaltet.
refused-operator-cannot-be-locked = Wer die Instanz verwaltet, kann nicht gesperrt werden.
refused-lock-reason-blank = Ein Konto zu sperren braucht einen Grund.
refused-lock-reason-control-character = Ein Grund darf keine Steuerzeichen enthalten.
refused-lock-reason-too-long = Ein Grund darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-account-id-invalid = Das ist keine gültige Konto-ID.
refused-account-not-found = Dieses Konto gibt es nicht.
instance-sections = Bereiche
instance-accounts-heading = Konten
instance-accounts-more = Mehr zeigen
instance-account-signed-up = Registriert am { $when }
instance-account-unverified = Adresse nicht bestätigt
instance-invitations-heading = Einladungen
instance-invite-email = E-Mail-Adresse
instance-invite-confirm = Einladen
instance-invitation-withdraw = Zurückziehen
instance-invitation-invite-again = Erneut einladen
instance-invitations-none = Gerade ist niemand eingeladen.
instance-invitations-more = Mehr Einladungen zeigen
instance-invitation-issued = Eingeladen am { $when }
instance-invitation-open = Offen bis { $when }
instance-invitation-expired = Abgelaufen am { $when }
instance-invitation-declined-address-has-account = Nicht nötig: Zu dieser E-Mail-Adresse gibt es schon ein Konto
instance-invitation-declined-turned-down = Abgelehnt
instance-account-operator = Verwaltet die Instanz
instance-account-locked = Gesperrt seit { $when }
instance-account-lock = Sperren …
instance-account-unlock = Entsperren
instance-account-leaving = Wird am { $when } gelöscht
instance-account-delete = Konto löschen …
instance-account-keep = Dieses Konto behalten
instance-delete-dialog = Das Konto von { $name } löschen?
instance-delete-hint = { $name } wird überall abgemeldet und kann sich nicht anmelden. Das Konto bleibt 30 Tage erhalten, bis zum { $until }, und bis dahin kannst du es behalten. Danach wird es gelöscht, zusammen mit den Projekten, in denen nur diese Person Mitglied ist. Stell vorher außerhalb von Weaveling sicher, dass das gewollt ist. Der Verlauf des Kontos zeigt, dass du das getan hast.
instance-delete-hand-over = { $name } ist die letzte Person, der Projekte gehören, in denen noch andere Mitglied sind. Sie muss sie zuerst übergeben.
instance-delete-cancel = Abbrechen
instance-delete-confirm = Konto löschen
instance-lock-dialog = { $name } sperren
instance-lock-hint = { $name } wird sofort abgemeldet und kann sich erst wieder anmelden, wenn das Konto entsperrt ist. Der Grund wird mit der Sperre gespeichert und hier angezeigt.
instance-lock-reason = Grund
instance-lock-cancel = Abbrechen
instance-lock-confirm = Sperren
refused-last-operator = Wer die Instanz als Letzte oder Letzter verwaltet, kann die Verwaltung nicht abgeben, sonst könnte niemand mehr diese Weaveling-Instanz verwalten.
refused-operator-address-not-verified = Nur ein Konto mit bestätigter Adresse kann die Instanz verwalten.
refused-locked-cannot-become-operator = Ein gesperrtes Konto kann die Instanz nicht verwalten.
refused-signing-up-by-invitation-only = Bei dieser Weaveling-Instanz kann man sich nur auf Einladung registrieren.
refused-invitation-invalid = Dieser Einladungslink ist ungültig oder funktioniert nicht mehr.
refused-invitation-expired = Diese Einladung ist abgelaufen. Bitte um eine neue.
refused-invitation-paused = Das Projekt dieser Einladung liegt im Papierkorb. Die Einladung lässt sich annehmen, sobald das Projekt wiederhergestellt ist.
refused-email-address-has-account = Zu dieser E-Mail-Adresse gibt es schon ein Konto. Melde dich stattdessen an.
refused-invitation-for-another-address = Diese Einladung ging an eine andere Adresse. Melde dich mit dieser Adresse an, um sie anzunehmen.
refused-invitation-not-by-an-operator = Bei dieser Weaveling-Instanz kann man sich nur auf Einladung registrieren. Bitte den Betreiber, dich einzuladen.
refused-totp-unavailable = Diese Weaveling-Instanz unterstützt die Anmeldung in zwei Schritten noch nicht. Bitte den Betreiber, sie einzurichten.
refused-totp-nothing-being-set-up = Das Einrichten der Anmeldung in zwei Schritten muss neu beginnen.
refused-totp-code-wrong = Das ist nicht der Code, den deine App zeigt. Warte auf den nächsten und versuche es noch einmal.
refused-recovery-code-wrong = Das ist keiner deiner Wiederherstellungscodes, oder er wurde schon benutzt.
refused-no-second-factor = Dieses Konto hat keine Anmeldung in zwei Schritten.
refused-keys-unavailable = Diese Weaveling-Instanz kann noch keine Schlüssel hinzufügen. Bitte den Betreiber, ihre öffentliche Adresse festzulegen.
refused-no-key-being-added = Das Hinzufügen des Schlüssels muss neu beginnen.
refused-no-key-being-asked = Die Abfrage des Schlüssels muss neu beginnen.
refused-key-refused = Aus der Antwort deines Browsers ließ sich kein Schlüssel für diese Weaveling-Instanz erstellen. Versuche es noch einmal.
refused-key-held-already = Dieser Schlüssel gehört schon zu einem Konto.
refused-no-such-key = Dieses Konto hat keinen solchen Schlüssel.
refused-key-id-invalid = Das ist keine gültige Schlüssel-ID.
refused-key-name-taken = Einer deiner anderen Schlüssel heißt schon so.
refused-key-name-blank = Ein Schlüssel braucht einen Namen.
refused-key-name-control-character = Der Name eines Schlüssels darf keine Steuerzeichen enthalten.
refused-key-name-too-long = Der Name eines Schlüssels darf höchstens { $max } Zeichen lang sein, dieser hat { $actual }.
refused-too-many-keys = Ein Konto hat höchstens 20 Schlüssel. Entferne zuerst einen.
refused-password-wrong = Das ist nicht dein Passwort.
refused-projects-to-hand-over = Du bist die letzte Person, der Projekte gehören, in denen noch andere Mitglied sind. Lass jedes davon zuerst noch jemand anderem gehören.
refused-no-deletion-pending = Für dieses Konto steht keine Löschung an.
refused-deletion-asked-by-the-operator = Wer diese Weaveling-Instanz verwaltet, hat die Löschung dieses Kontos veranlasst. Deshalb kann nur diese Person sie zurücknehmen.
refused-preferred-way-unknown = Wähle die App, einen Schlüssel oder den Weg, den du zuletzt genutzt hast.
refused-key-role-unknown = Ein Schlüssel ist entweder ein Passkey oder ein zweiter Schritt.
instance-account-appoint = Zum Verwalter ernennen
instance-account-dismiss = Als Verwalter abberufen
instance-appoint-dialog = { $name } zum Verwalter ernennen
instance-appoint-hint = { $name } kann diese Weaveling-Instanz dann wie du verwalten: die Konten auflisten, sperren und entsperren und andere zum Verwalter ernennen und abberufen.
instance-appoint-confirm = Ernennen
instance-dismiss-dialog = { $name } als Verwalter abberufen
instance-dismiss-hint = { $name } kann diese Weaveling-Instanz dann nicht mehr verwalten.
instance-dismiss-yourself-hint = Du kannst diese Weaveling-Instanz dann nicht mehr verwalten, und nur wer sie weiter verwaltet, kann dir die Verwaltung zurückgeben.
instance-dismiss-confirm = Abberufen
instance-role-cancel = Abbrechen
refused-public-url-missing = Diese Weaveling-Instanz weiß nicht, wo sie erreichbar ist, also führten ihre Einladungen nirgendwohin. Bitte die Person, die sie verwaltet, ihre öffentliche Adresse festzulegen.
refused-invitation-id-invalid = Das ist keine gültige Einladungs-ID.
refused-invitation-not-found = Diese Einladung gibt es nicht.
refused-invitation-already-accepted = Diese Einladung wurde schon angenommen.
refused-too-late-to-try-again = Diese Änderung ist zu alt, um sie noch einmal zu versuchen.

members-link = Mitglieder
members-link-of = Mitglieder: { $names }
share = Teilen
share-title = „{ $project }“ teilen
share-people = Wer Zugang hat
share-invited = Eingeladen
share-all-members = Alle Mitglieder und Einladungen
members-heading = Mitglieder
members-hint = Wer dieses Projekt lesen und schreiben darf.
members-you = (du)
members-unnamed = Name noch unbekannt
members-role-owner = Eigentümer:in
members-role-author = Autor:in
members-role-reader = Leser:in
roles-changed = Deine Rolle wurde zu { $role } geändert.
members-roles = Eigentümer:innen laden ein, ändern Rollen und löschen das Projekt. Autor:innen lesen und schreiben. Leser:innen lesen nur.
members-role-of = Rolle von { $name }
members-remove = Entfernen
members-leave = Verlassen
members-sections = Bereiche
members-people = Personen
members-invited = Eingeladen
members-invite-as = Als
members-only-owner = Einzige:r Eigentümer:in
members-last-owner-heading = Du bist einzige:r Eigentümer:in
members-last-owner-hint = Bevor du gehst, mach jemand anderen zur Eigentümer:in. Sonst könnte niemand mehr einladen, Rollen ändern oder dieses Projekt in den Papierkorb verschieben.
members-new-owner = Neue:r Eigentümer:in
members-candidate = { $name } ({ $role })
members-stay = Bleiben
members-hand-over = Übergeben und verlassen
members-alone-heading = Sonst ist niemand dabei
members-alone-hint = Dieses Projekt hat keine anderen Mitglieder. Würdest du es verlassen, wäre niemand mehr darin. Verschieb es stattdessen in den Papierkorb, wo es sich 30 Tage lang wiederherstellen lässt, oder lade jemanden ein und übergib es zuerst.
members-trash-project = Projekt in den Papierkorb verschieben
