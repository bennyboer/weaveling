masthead-all-projects = Todos los proyectos
masthead-views = Vistas
view-write = Escribir
view-board = Tablero
view-outline = Esquema
view-ideas = Ideas
status-bar = Estado

theme = Tema
theme-light = Claro
theme-system = Según el sistema
theme-dark = Oscuro
theme-chosen = Tema: { $theme }

language = Idioma
language-machine-translated = { $language } (traducción automática)

workspace-heading = Tus proyectos
workspace-tagline = Tráenos tus pequeñas y frágiles ideas para historias, y te ayudaremos a tejerlas hasta formar una gran epopeya.
loading = Cargando…
workspace-empty = Todavía no hay proyectos. Toda epopeya empieza siendo un pequeño desorden.

new-project-working-title = Un título provisional…
new-project-create = Crear
new-project-dialog = Nuevo proyecto
new-project-confirm = Crear proyecto

project-more-actions = Más acciones
project-rename = Cambiar el nombre
project-delete = Eliminar
project-rename-save = Guardar
project-rename-cancel = Cancelar

delete-project-dialog = ¿Eliminar este proyecto?
delete-project-fate = «{ $name }» desaparecerá para siempre. Eliminarlo no se puede deshacer.
delete-project-cancel = Cancelar
delete-project-confirm = Eliminar

new-project-cancel = Cancelar

details-title = Título
details-book-language = Idioma del libro
details-book-language-unset = Sin indicar
details-book-language-hint = El idioma en que está escrito el libro, usado allí donde su texto depende de él.
details-quotation = Comillas
details-quotation-as-the-language-has-it = Las del idioma, { $marks }
details-quotation-hint = Los signos alrededor de lo que se dice, puestos al escribir ".
details-description = Descripción
details-description-placeholder = De qué trata el libro, en pocas líneas. Opcional.

book-language-de = Alemán (Deutsch)
book-language-en = Inglés (English)
book-language-fr = Francés (Français)
book-language-es = Español
book-language-it = Italiano
book-language-nl = Neerlandés (Nederlands)
book-language-pt = Portugués (Português)
book-language-pl = Polaco (Polski)
book-language-sv = Sueco (Svenska)
book-language-da = Danés (Dansk)
book-language-nb = Noruego (Norsk)
book-language-fi = Finés (Suomi)
book-language-cs = Checo (Čeština)

project-details = Detalles del proyecto
project-details-cancel = Cancelar
project-details-save = Guardar
project-details-changing-the-marks = Cambiar las comillas de { $from } a { $to } las reescribe en todas las escenas del libro. En un libro largo, eso lleva un rato.
project-details-back = Volver
project-details-change-the-marks = Cambiar las comillas

restyling = Reescribiendo las comillas
restyling-waiting = Reescribiendo las comillas…
restyling-running = Reescribiendo las comillas: { $done } de { $total } { $total ->
        [one] escena
       *[other] escenas
    }.
restyling-finished = Todas las escenas usan ahora { $marks }.
restyling-close = Cerrar

problem-not-found =
    { $subject ->
        [appearances] Esas apariciones ya no existen.
        [board] Ese tablero ya no existe.
        [idea] Esa idea ya no existe.
        [outline] Ese esquema ya no existe.
        [project] Ese proyecto ya no existe.
        [refusal] Ese rechazo ya no existe.
        [scene] Esa escena ya no existe.
        [word-counts] Esos recuentos de palabras ya no existen.
       *[other] Eso ya no existe.
    }
problem-outdated =
    { $subject ->
        [appearances] Estas apariciones se cambiaron en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo están.
        [board] Este tablero se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo está.
        [idea] Esta idea se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirla para ver cómo está.
        [outline] Este esquema se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo está.
        [project] Este proyecto se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo está.
        [refusal] Este rechazo se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo está.
        [scene] Esta escena se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirla para ver cómo está.
        [word-counts] Estos recuentos de palabras se cambiaron en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo están.
       *[other] Esto se cambió en otro lugar mientras tanto. Cierra esto y vuelve a abrirlo para ver cómo está.
    }
problem-overtaken = Alguien más lo cambió mientras tanto, así que no se pudo hacer.
problem-offline = No se pudo conectar con el servidor. Inténtalo de nuevo en un momento.
problem-unexpected = Algo inesperado salió mal. Inténtalo de nuevo.
problem-dismiss = Descartar

refused-project-id-invalid = Eso no es un ID de proyecto válido.
refused-project-name-blank = Un proyecto necesita un título.
refused-project-name-control-character = Un título no puede contener caracteres de control.
refused-project-name-too-long = Un título puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-book-language-not-a-tag = «{ $tag }» no es una etiqueta de idioma como «de», «en» o «en-GB».
refused-quotation-marks-unknown = «{ $name }» no son comillas que Weaveling conozca.
refused-project-description-control-character = Una descripción no puede contener caracteres de control, salvo saltos de línea.
refused-project-description-too-long = Una descripción puede tener como máximo { $max } caracteres; esta tiene { $actual }.
refused-project-deleted = Este proyecto se eliminó mientras tanto.
refused-board-id-invalid = Eso no es un ID de tablero válido.
refused-idea-already-pinned = Esta idea ya está en el tablero.
refused-idea-not-pinned = Esta idea no está en el tablero.
refused-card-shapeless = Una tarjeta necesita ancho y alto.
refused-board-discarded = Este tablero se descartó mientras tanto.
refused-idea-id-invalid = Eso no es un ID de idea válido.
refused-idea-title-control-character = El título de una idea no puede contener caracteres de control.
refused-idea-title-too-long = El título de una idea puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-idea-discarded = Esta idea se descartó mientras tanto.
refused-outline-id-invalid = Eso no es un ID de esquema válido.
refused-section-title-control-character = El título de una sección no puede contener caracteres de control.
refused-section-title-too-long = El título de una sección puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-outline-holds-no-such-kind = Un esquema contiene escenas e ideas, y nada más.
refused-section-not-found = Esta sección ya no está en el esquema.
refused-section-would-contain-itself = Una sección no puede moverse dentro de sí misma.
refused-section-neighbour-not-found = En ese lugar no hay nada detrás de lo que ponerlo.
refused-not-in-the-outline = Esto no está en el esquema.
refused-outline-discarded = Este esquema se descartó mientras tanto.
refused-scene-id-invalid = Eso no es un ID de escena válido.
refused-scene-title-control-character = El título de una escena no puede contener caracteres de control.
refused-scene-title-too-long = El título de una escena puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-quotation-never-restyled = Las comillas de este libro nunca se reescribieron.

untitled = Sin título
board = Tablero
board-not-on-it = Fuera del tablero · { $count }
board-waiting = Ideas fuera del tablero
board-every-idea-pinned = Todas las ideas están en el tablero.
board-how-to-pin = Haz clic en una idea para fijarla donde haya sitio.
board-pin = Fijar { $idea }
board-actions = Acciones para { $idea }
board-rename = Cambiar el nombre de { $idea }
board-open = Abrir { $idea }
board-unpin = Quitar { $idea } del tablero
idea-asked = ¿Cuál es la idea?
board-zoom = Zoom
board-zoom-out = Alejar
board-zoom-reset = Restablecer el zoom
board-zoom-in = Acercar
tray = Bandeja
inspector = Inspector
inspector-idea-name = Nombre de la idea
inspector-appears-in = Aparece en
inspector-appears-nowhere = Todavía no está en el libro: ninguna sección la anota, ninguna escena se inspira en ella.
board-resize =
    { $side ->
        [top] Cambiar el tamaño de { $idea } desde arriba
        [bottom] Cambiar el tamaño de { $idea } desde abajo
        [left] Cambiar el tamaño de { $idea } desde la izquierda
        [right] Cambiar el tamaño de { $idea } desde la derecha
        [top-left] Cambiar el tamaño de { $idea } desde arriba a la izquierda
        [top-right] Cambiar el tamaño de { $idea } desde arriba a la derecha
        [bottom-left] Cambiar el tamaño de { $idea } desde abajo a la izquierda
        [bottom-right] Cambiar el tamaño de { $idea } desde abajo a la derecha
       *[other] Cambiar el tamaño de { $idea }
    }
drawer-close =
    { $drawer ->
        [tray] Cerrar la bandeja
        [contents] Cerrar el índice
       *[other] Cerrar
    }
drawer-resize =
    { $drawer ->
        [tray] Cambiar el ancho de la bandeja
        [contents] Cambiar el ancho del índice
       *[other] Cambiar el ancho
    }

scene-empty = Vacía
outline-manuscript = Manuscrito
outline-the-manuscript = El manuscrito
outline-nothing-yet = Todavía no hay nada en el libro. Añade una sección para empezar.
outline-add-section = Añadir una sección
outline-not-in-the-book = Fuera del libro · { $count }
outline-scenes = Escenas
outline-ideas = Ideas
outline-fold = Plegar { $section }
outline-section = Sección { $section }
outline-actions = Acciones para { $section }
outline-move = Mover { $section }
outline-move-earlier = Adelantar { $section }
outline-move-later = Atrasar { $section }
outline-promote = Subir de nivel { $section }
outline-demote = Bajar de nivel { $section }
outline-write-in = Escribir en { $section }
outline-remove = Quitar { $section }
outline-take-out = Sacar { $name } del libro
outline-scenes-waiting = Escenas fuera del libro
outline-every-scene-placed = Cada escena tiene su sitio en el libro.
outline-ideas-waiting = Ideas fuera del libro
outline-place = Colocar { $name }
outline-how-to-place = Arrástrala a una sección, o haz clic en ella y luego donde va. Una escena pasa a ser el libro; una idea, una nota a su lado.
renamed-meanwhile = Renombrado mientras tanto
renamed-meanwhile-told = Mientras editabas este título, alguien lo renombró a «{ $theirs }». El tuyo es «{ $mine }».
renamed-meanwhile-theirs = Quedarse con el suyo
renamed-meanwhile-mine = Mantener el mío

ideas = Ideas
ideas-capture = Apuntar
ideas-empty = Todavía no hay ideas. Lanza una idea y mira adónde lleva.
idea-opening = Abriendo…

book = El libro
writing-sides = Lados de la obra
writing-side-book = El libro
writing-side-loose = Fuera del libro
writing-every-scene-placed = Todas las escenas están en el libro.
heading-untitled = Sección sin título
heading-first = La primera sección de tu libro
heading-next = Una sección nueva
scene-invitation = Una escena vacía. Empieza a escribir…
scene-next = Una escena nueva
scene-an-empty-one = Una escena vacía
status-focus = Modo concentración
status-focus-hint = Modo concentración (Ctrl+Mayús+F)
writing-settings = Ajustes de escritura
count-whole-book = Contar todo el libro
count-section = Contar la sección en la que escribes
digit-group = { "." }
words =
    { $count ->
        [one] { $shown } palabra
       *[other] { $shown } palabras
    }
words-not-in-the-book = { $words } fuera del libro
words-in-the-book = { $words } en el libro
words-in-section = { $words } en «{ $section }»
words-in-this-section = { $words } en esta sección
saving = Guardando…
all-saved = Todo guardado
setting-spelling = Revisar la ortografía
setting-spelling-hint = Subraya las palabras que el diccionario del navegador no conoce.
setting-justified = Texto justificado
setting-justified-hint = Alinea las líneas por ambos lados como una página impresa y divide las palabras en el idioma del libro.
setting-typewriter = Desplazamiento de máquina de escribir
setting-typewriter-hint = Mantiene la línea que escribes en el centro de la ventana.
settings-close = Cerrar
contents = Índice
this-scene = Esta escena
where-you-are = Dónde estás
quiet-rearranged = Alguien más reorganizó el libro en torno a «{ $section }».
quiet-removed = Alguien más quitó «{ $section }».
quiet-overtaken = Alguien más cambió el libro mientras tanto, así que no se pudo hacer. Muestra el libro tal como está ahora.
going-back = Volver a donde lo dejaste

move-scene = Mover la escena
drag-to-move = Arrastrar para mover
scene-actions = Acciones de la escena
menu-rename = Cambiar el nombre
menu-take-out = Sacar del libro
menu-bring-in = Llevar al libro…
menu-delete-scene = Eliminar la escena
section-actions = Acciones de la sección
menu-nest = Anidar bajo la sección anterior
menu-lift = Subir un nivel
menu-remove-section = Quitar la sección
add-here = Añadir aquí
menu-new-scene = Escena nueva
menu-new-section = Sección nueva
menu-new-top-level-section = Sección nueva de primer nivel
menu-bring-back = Recuperar una escena…
scene-title = Título de la escena
menu-button-title = { $menu } ({ $keys })
keys-tab = Tab
keys-shift-tab = Mayús+Tab
keys-shift-f10 = Mayús+F10
cancel = Cancelar
asked-keep-writing = Seguir escribiendo
answer-delete = Eliminar
answer-remove = Quitar
answer-show-it = Mostrarla
answer-discard = Descartar
answer-put-at-end = Ponerla al final del libro
answer-take-out = Sacar del libro
answer-delete-for-good = Eliminar para siempre
asked-delete-scene = ¿Eliminar esta escena?
asked-delete-scene-told-unnamed = Esta escena y todo lo escrito en ella desaparecerán para siempre. Eliminarla no se puede deshacer.
asked-delete-scene-told = «{ $scene }» y todo lo escrito en ella desaparecerán para siempre. Eliminarla no se puede deshacer.
removed-meanwhile = Quitada mientras tanto
removed-meanwhile-told-unnamed = Alguien más quitó «{ $section }». Tu escena está ahora fuera del libro, entre las escenas que no están en él.
removed-meanwhile-told = Alguien más quitó «{ $section }». Tu escena «{ $scene }» está ahora fuera del libro, entre las escenas que no están en él.
place-gone = Su sitio ya no está
place-gone-scene = Alguien más quitó el lugar al que iba tu escena nueva. Empieza «{ $begins }».
place-gone-section = Alguien más quitó aquello tras lo que iba tu sección nueva «{ $section }».
renamed-meanwhile-heading-told = Mientras editabas este encabezado, alguien lo renombró a «{ $theirs }». El tuyo es «{ $mine }».
asked-remove-section = ¿Quitar esta sección?
asked-remove-section-told = Sus escenas salen del libro pero no se eliminan: esperan entre las escenas que no están en el libro. Las secciones de dentro suben a ocupar su lugar.
asked-remove-chosen = ¿Quitar lo elegido?
chosen-scenes =
    { $scenes ->
        [one] { $scenes } escena elegida.
       *[other] { $scenes } escenas elegidas.
    }
chosen-sections =
    { $sections ->
        [one] { $sections } sección elegida.
       *[other] { $sections } secciones elegidas.
    }
chosen-both =
    { $scenes ->
        [one] { $scenes } escena
       *[other] { $scenes } escenas
    } y { $sections ->
        [one] { $sections } sección
       *[other] { $sections } secciones
    } elegidas.
chosen-whole = Cada sección se va con las secciones que contiene y todas sus escenas.
chosen-ways-book = Sacadas del libro, las escenas esperan entre las que no están en él. Eliminadas para siempre, ellas y todo lo escrito en ellas desaparecen, y eso no se puede deshacer.
chosen-ways-loose = Eliminadas para siempre, ellas y todo lo escrito en ellas desaparecen, y eso no se puede deshacer.
placing = Llevar al libro
placing-told-unnamed = La escena va al final de la sección que elijas.
placing-told = «{ $scene }» va al final de la sección que elijas.
placing-sections = Secciones del libro
bring-back = Recuperar una escena
looking = Buscando…
bring-back-none = Todas las escenas ya están en el libro.
bring-back-told = Estas escenas no están en el libro. La que elijas entra donde hiciste clic.
bring-back-find = Buscar una escena
bring-back-no-match = Ninguna escena coincide.

contents-unfold-all = Desplegar todo
contents-fold-all = Plegar todo
contents-unfold = Desplegar { $section }
this-section = Esta sección
inspector-nothing-yet = Pon el cursor en una escena o un encabezado para verlo aquí.
inspector-section-removed = Esta sección se quitó.
pinned-ideas = Ideas anotadas
pinned-none = Aquí no hay ideas anotadas.
pin-an-idea = Anotar una idea
section-unpin = Quitar { $idea }
drawn-from = Se inspira en
drawn-from-none = Todavía no hay ideas enlazadas.
link-an-idea = Enlazar una idea
unlink-idea = Desenlazar { $idea }
pin-ideas = Anotar ideas
ideas-to-pin = Ideas para anotar
every-idea-pinned = Todas las ideas de este proyecto ya están anotadas en el esquema.
pin-ideas-confirm =
    { $count ->
        [0] Anotar
        [1] Anotar
       *[other] Anotar { $count }
    }
link-ideas = Enlazar ideas
ideas-to-link = Ideas para enlazar
every-idea-linked = Todas las ideas de este proyecto ya están enlazadas.
link-ideas-confirm =
    { $count ->
        [0] Enlazar
        [1] Enlazar
       *[other] Enlazar { $count }
    }
find-an-idea = Buscar una idea
no-idea-matches = Ninguna idea coincide.

refusals = Cambios que no se pudieron aplicar
refusals-told = Se guardaron, pero no llegaron a todos los sitios a los que debían. Vuelve a intentarlo cuando se arregle lo que los detuvo, o confírmalos para acallar la alarma.
refusals-none = No queda nada que mirar.
refusals-list = Cambios rechazados
close = Cerrar
refusal-wordless = Un cambio no llegó a todos los sitios a los que debía.
refusal-given-up = Abandonado el { $when }
refusal-details = Detalles
refusal-listener = Receptor
refusal-message = Mensaje
refusal-attempts = Intentos
refusal-reason = Motivo
refusal-retry = Volver a intentar
refusal-acknowledge = Confirmar
alarm-note = Algunos cambios no se pudieron aplicar.
alarm-show = Mostrar
missing = No hay nada tejido en esta dirección.
missing-back = Volver a tus proyectos

moment =
    { $day } { $month ->
        [1] ene
        [2] feb
        [3] mar
        [4] abr
        [5] may
        [6] jun
        [7] jul
        [8] ago
        [9] sept
        [10] oct
        [11] nov
       *[12] dic
    } { $year }, { $hour }:{ $minute }

sign-in-heading = Iniciar sesión
sign-in-email = Correo electrónico
sign-in-password = Contraseña
sign-in-confirm = Iniciar sesión
sign-out = Cerrar sesión
refused-email-malformed = Eso no es un correo electrónico como nombre@ejemplo.com.
refused-email-too-long = Un correo electrónico puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-account-name-blank = Una cuenta necesita un nombre.
refused-account-name-control-character = Un nombre no puede contener caracteres de control.
refused-account-name-too-long = Un nombre puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-password-too-short = Una contraseña necesita al menos { $min } caracteres; esta tiene { $actual }.
refused-password-too-long = Una contraseña puede tener como máximo { $max } caracteres; esta tiene { $actual }.
refused-verification-code-wrong = Ese no es el código que enviamos.
refused-verification-code-expired = Este código ha caducado. Pide uno nuevo.
refused-verification-code-exhausted = Este código se intentó demasiadas veces. Pide uno nuevo.
refused-sign-in-refused = Con este correo electrónico y esta contraseña no se puede iniciar sesión.
refused-address-not-verified = Este correo electrónico aún no está confirmado. Introduce el código que te enviamos.
refused-not-signed-in = Primero inicia sesión.
sign-up-heading = Crear cuenta
sign-up-name = Tu nombre
sign-up-password-hint = Al menos 12 caracteres.
sign-up-confirm = Crear cuenta
sign-up-instead = ¿Aún no tienes cuenta? Créala
sign-in-instead = ¿Ya tienes cuenta? Inicia sesión
code-heading = Confirma tu correo
code-sent = Enviamos un código a { $email }.
code-field = Código
code-confirm = Confirmar
code-again = Enviar un código nuevo
code-sent-again = Un código nuevo va en camino.
code-confirmed = Tu correo está confirmado. Ya puedes iniciar sesión.
code-enter = Introducir el código
unreachable-heading = No se puede conectar con el servidor
try-again = Intentar de nuevo
account-menu = Cuenta: { $name }
reset-instead = ¿Olvidaste tu contraseña?
reset-heading = Elegir una contraseña nueva
reset-ask = Enviar un código
reset-password = Contraseña nueva
reset-confirm = Guardar la contraseña nueva
reset-done = Tu contraseña nueva está guardada. Ya puedes iniciar sesión con ella.
back-to-sign-in = Volver a iniciar sesión
refused-too-many-tries =
    { $minutes ->
        [one] Demasiados intentos. Espera un minuto antes de volver a intentarlo.
       *[other] Demasiados intentos. Espera { $minutes } minutos antes de volver a intentarlo.
    }
