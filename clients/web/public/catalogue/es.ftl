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

language = Idioma
masthead-more = Más
masthead-this-project = Este proyecto
language-machine-translated = { $language } (traducción automática)

workspace-heading = Tus proyectos
workspace-tagline = Tráenos tus pequeñas y frágiles ideas para historias, y te ayudaremos a tejerlas hasta formar una gran epopeya.
loading = Cargando…
workspace-empty = Todavía no hay proyectos. Toda epopeya empieza siendo un pequeño desorden.
workspace-trash-link = Papelera, { $count } { $count ->
        [one] proyecto
       *[other] proyectos
    }

new-project-working-title = Un título provisional…
new-project-create = Crear
new-project-dialog = Nuevo proyecto
new-project-confirm = Crear proyecto
new-project-setting-up = Preparando tu proyecto…

project-more-actions = Más acciones
project-rename = Cambiar el nombre
project-trash = Mover a la papelera
project-rename-save = Guardar
project-rename-cancel = Cancelar

trash-project-dialog = ¿Mover este proyecto a la papelera?
trash-project-fate = «{ $name }» se puede restaurar desde la papelera durante 30 días. Después desaparecerá para siempre.
trash-project-cancel = Cancelar
trash-project-confirm = Mover a la papelera
trash-heading = Papelera
trash-empty = La papelera está vacía.
trash-kept-until = Se puede restaurar hasta el { $moment }
trash-restore = Restaurar

new-project-cancel = Cancelar

details-title = Título
details-book-language = Idioma del libro
details-book-language-unset = Sin indicar
details-book-language-hint = El idioma en que está escrito tu libro. La división de palabras y las comillas siguen este idioma.
details-quotation = Comillas
details-quotation-as-the-language-has-it = Las predeterminadas del idioma, { $marks }
details-quotation-hint = Los signos que se ponen alrededor de los diálogos al escribir ".
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
        [account] Esa cuenta ya no existe.
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
        [appearances] Otra persona cambió estas apariciones mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
        [board] Otra persona cambió este tablero mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
        [idea] Otra persona cambió esta idea mientras tanto. Cierra esto y vuelve a abrirla para ver la versión más reciente.
        [outline] Otra persona cambió este esquema mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
        [project] Otra persona cambió este proyecto mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
        [refusal] Otra persona cambió este rechazo mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
        [scene] Otra persona cambió esta escena mientras tanto. Cierra esto y vuelve a abrirla para ver la versión más reciente.
        [word-counts] Otra persona cambió estos recuentos de palabras mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
       *[other] Otra persona cambió esto mientras tanto. Cierra esto y vuelve a abrirlo para ver la versión más reciente.
    }
problem-overtaken = Alguien más lo cambió mientras tanto, así que no se pudo hacer.
problem-offline = No se pudo conectar con el servidor. Inténtalo de nuevo en un momento.
problem-unexpected = Algo inesperado salió mal. Inténtalo de nuevo.
problem-dismiss = Descartar

refused-project-id-invalid = Eso no es un ID de proyecto válido.
refused-project-name-blank = Un proyecto necesita un título.
refused-project-name-control-character = Un título no puede contener caracteres de control.
refused-project-name-too-long = Un título puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-project-not-yet-open = Tu proyecto se ha creado, pero todavía no se puede abrir. Aparecerá en tu lista en un momento.
refused-book-language-not-a-tag = «{ $tag }» no es una etiqueta de idioma como «de», «en» o «en-GB».
refused-quotation-marks-unknown = Weaveling no conoce las comillas «{ $name }».
refused-project-description-control-character = Una descripción no puede contener caracteres de control, salvo saltos de línea.
refused-project-description-too-long = Una descripción puede tener como máximo { $max } caracteres; esta tiene { $actual }.
refused-project-deleted = Este proyecto se ha eliminado mientras tanto.
refused-project-trashed = Este proyecto se ha movido a la papelera mientras tanto.
refused-project-not-trashed = Este proyecto no está en la papelera.
refused-not-an-owner = Solo quien es propietario de este proyecto puede hacer esto.
refused-last-owner = Un proyecto necesita al menos a una persona propietaria. Haz propietaria a otra persona primero.
refused-not-a-member = Esta persona no es miembro de este proyecto.
refused-role-unknown = Ese rol no existe en un proyecto.
refused-member-leaving = Esta cuenta está a punto de eliminarse, así que su rol no se puede cambiar.
refused-not-an-author = Solo quien escribe en este proyecto puede hacer esto.
refused-board-id-invalid = Eso no es un ID de tablero válido.
refused-idea-already-pinned = Esta idea ya está en el tablero.
refused-idea-not-pinned = Esta idea no está en el tablero.
refused-card-shapeless = Una tarjeta necesita ancho y alto.
refused-board-discarded = Este tablero se ha descartado mientras tanto.
refused-idea-id-invalid = Eso no es un ID de idea válido.
refused-idea-title-control-character = El título de una idea no puede contener caracteres de control.
refused-idea-title-too-long = El título de una idea puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-idea-discarded = Esta idea se ha descartado mientras tanto.
refused-outline-id-invalid = Eso no es un ID de esquema válido.
refused-section-title-control-character = El título de una sección no puede contener caracteres de control.
refused-section-title-too-long = El título de una sección puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-outline-holds-no-such-kind = Un esquema contiene escenas e ideas, y nada más.
refused-section-not-found = Esta sección ya no está en el esquema.
refused-section-would-contain-itself = Una sección no puede moverse dentro de sí misma.
refused-section-neighbour-not-found = El elemento al que debía seguir ya no está.
refused-not-in-the-outline = Esto no está en el esquema.
refused-outline-discarded = Este esquema se ha descartado mientras tanto.
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
inspector-appears-nowhere = Todavía no está en el libro. Ninguna sección ni escena la usa.
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
outline-how-to-place = Arrástrala a una sección, o haz clic en ella y luego donde debe ir. Una escena pasa a formar parte del libro. Una idea pasa a ser una nota a su lado.
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
path = Ruta
path-home = Inicio
path-invitation = Invitación
path-missing = No encontrado
quiet-rearranged = Alguien más reorganizó el libro en torno a «{ $section }».
quiet-removed = Alguien más quitó «{ $section }».
quiet-overtaken = Alguien más cambió el libro mientras tanto, así que no se pudo hacer. Ahora ves la versión actual.
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
removed-meanwhile-told-unnamed = Alguien más quitó «{ $section }». Tu escena está ahora entre las escenas que no están en el libro.
removed-meanwhile-told = Alguien más quitó «{ $section }». Tu escena «{ $scene }» está ahora entre las escenas que no están en el libro.
place-gone = Su sitio ya no está
place-gone-scene = Alguien más quitó el lugar al que iba tu escena nueva. Empieza «{ $begins }».
place-gone-section = Alguien más quitó el elemento detrás del que iba tu sección nueva «{ $section }».
renamed-meanwhile-heading-told = Mientras editabas este encabezado, alguien lo renombró a «{ $theirs }». El tuyo es «{ $mine }».
asked-remove-section = ¿Quitar esta sección?
asked-remove-section-told = Sus escenas salen del libro, pero no se eliminan. Las encontrarás entre las escenas que no están en el libro. Las secciones de dentro suben un nivel.
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
chosen-whole = Cada sección se quita junto con las secciones que contiene y todas sus escenas.
chosen-ways-book = Si las sacas del libro, las escenas pasan a las escenas que no están en el libro. Si las eliminas, ellas y todo lo escrito en ellas desaparecen para siempre. Esto no se puede deshacer.
chosen-ways-loose = Si las eliminas, ellas y todo lo escrito en ellas desaparecen para siempre. Esto no se puede deshacer.
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
refusals-told = Estos cambios se guardaron, pero no todas las partes de Weaveling los recibieron. Arregla la causa y vuelve a intentarlo, o confírmalos para silenciar la alarma.
refusals-none = No queda nada que mirar.
refusals-list = Cambios rechazados
close = Cerrar
refusal-wordless = Un cambio no llegó a todas las partes de Weaveling.
refusal-given-up = Abandonado el { $when }
refusal-details = Detalles
refusal-listener = Receptor
refusal-message = Mensaje
refusal-attempts = Intentos
refusal-reason = Motivo
refusal-retry = Volver a intentar
refusal-too-old = Este cambio tiene más de 30 días y no se vuelve a intentar, porque puede que aquello de lo que trataba ya no exista. Confírmalo para silenciar la alarma.
refusal-acknowledge = Confirmar
alarm-note = Algunos cambios no se pudieron aplicar.
alarm-show = Mostrar
missing = No hay nada tejido en esta dirección.

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

day =
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
    } { $year }

deletion-pending-heading = Tu cuenta se va a eliminar
deletion-pending-asked = Lo pediste el { $asked }. El { $until } se eliminarán para siempre tu cuenta y los proyectos de los que solo tú eres miembro.
deletion-pending-nothing-lost = Hasta entonces no se pierde nada. Si conservas tu cuenta, sigues con la sesión iniciada y encuentras cada proyecto tal como lo dejaste.
deletion-pending-keep = Conservar mi cuenta
deletion-pending-let-go = Cerrar sesión y dejar que se elimine
deletion-pending-by-the-operator = El operador de esta instancia de Weaveling ha programado la eliminación de tu cuenta para el { $until }. Pregúntale si quieres conservarla.
deletion-pending-sign-out = Cerrar sesión
your-account-heading = Tu cuenta
your-account-security = Cómo inicias sesión:
your-account-delete-heading = Eliminar esta cuenta
your-account-delete-hint = Tu cuenta y todo lo que le pertenece se eliminan para siempre tras 30 días.
your-account-delete = Eliminar esta cuenta…
your-account-delete-question = ¿Eliminar tu cuenta?
your-account-delete-kept = Se cierra tu sesión en todas partes al instante. Tu cuenta y tu trabajo se conservan durante 30 días, hasta el { $until }. Inicia sesión antes para conservarlo todo. Después, desaparece para siempre.
your-account-delete-alone = Los proyectos de los que solo tú eres miembro también se eliminan.
your-account-delete-password = Tu contraseña, para estar seguros de que eres tú
your-account-keep = Conservar mi cuenta
your-account-delete-confirm = Eliminar mi cuenta
your-account-hand-over-heading = Entrega primero estos proyectos
your-account-hand-over-hint = Eres la única persona propietaria de proyectos de los que también son miembros otras personas. Haz propietaria a una de ellas o elimina el proyecto. Después podrás eliminar tu cuenta.
your-account-hand-over-members = Miembros
your-account-hand-over-close = Cerrar

sign-in-heading = Iniciar sesión
sign-in-email = Correo electrónico
sign-in-password = Contraseña
sign-in-confirm = Iniciar sesión
passkey-sign-in = Iniciar sesión con una llave de acceso
passkey-or-password = o con tu contraseña
passkey-not-touched = No se usó ninguna llave de acceso. Inténtalo otra vez o inicia sesión con tu contraseña.
passkey-failed = Tu navegador no pudo pedir una llave de acceso. Inicia sesión con tu contraseña.
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
refused-sign-in-refused = El correo electrónico o la contraseña no son correctos.
refused-address-not-verified = Este correo electrónico aún no está confirmado. Introduce el código que te enviamos.
refused-not-signed-in = Primero inicia sesión.
sign-up-heading = Crear cuenta
sign-up-name = Tu nombre
sign-up-password-hint = Al menos 12 caracteres.
sign-up-confirm = Crear cuenta
sign-up-instead = ¿Aún no tienes cuenta? Créala
sign-in-instead = ¿Ya tienes cuenta? Inicia sesión
set-up-instead = Configurar esta instancia de Weaveling
set-up-heading = Configurar esta instancia de Weaveling
set-up-hint = Esta instancia de Weaveling todavía no tiene operador. Crea una cuenta con el código del registro del servidor. En cuanto confirmes tu dirección, serás su operador.
set-up-code = Código del registro del servidor
set-up-done = Esta instancia de Weaveling ya está configurada y tiene operador. Un operador puede convertirte también en operador.
invitation-accept-heading = Has recibido una invitación
invitation-accept-hint = Elige tu nombre y una contraseña. Tu cuenta usará el correo electrónico al que llegó la invitación.
invitation-signed-in = Has iniciado sesión como { $email }. Acepta la invitación para unirte al proyecto.
invitation-accept = Aceptar
invitation-accepted = Invitación aceptada. El proyecto aparecerá en tu lista en un momento.
invitation-decline = Rechazar
invitation-declined = Rechazaste la invitación.
invitation-decline-instead = Rechazar la invitación
invitation-declined-heading = Invitación rechazada
invitation-declined-hint = La rechazaste. Quien te invitó lo verá, y el enlace ya no funciona.
notifications = Notificaciones
notifications-waiting = Notificaciones: { $count } nuevas
notifications-none = Ahora mismo no hay nada nuevo.
notifications-invitations = Invitaciones
notice-invitation-from = De { $by }
notice-invitation-unnamed-project = un proyecto
notice-invitation-join = Unirme
notice-invitation-decline = Rechazar
notice-invitation-joined = Te uniste a { $project }. Aparecerá en tu lista en un momento.
notice-invitation-declined = Rechazaste la invitación a { $project }.
notifications-roles = Roles
notice-role-unnamed-project = un proyecto
notice-role-changed-by = { $by } cambió tu rol en { $project } a { $role ->
        [owner] propietario/a
        [author] autor/a
       *[reader] lector/a
    }.
notice-role-changed = Tu rol en { $project } ahora es { $role ->
        [owner] propietario/a
        [author] autor/a
       *[reader] lector/a
    }.
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
refused-operator-code-wrong = Este no es el código que el servidor escribió en el registro.
account-menu-operator = Administrar instancia
path-account = Tu cuenta
security-heading = Seguridad
security-hint = Cómo inicias sesión en esta instancia de Weaveling.
security-second-step = Segundo paso
security-second-step-hint = Después de tu contraseña, Weaveling puede pedir un segundo paso, para que la contraseña sola no baste para iniciar sesión.
security-app = Aplicación de autenticación
security-app-on-since = Activada desde el { $when }
security-app-actions = Acciones para la aplicación de autenticación
security-codes = Códigos de recuperación
security-codes-left = Quedan { $left } de 10
security-set-up-app = Configurar una aplicación de autenticación
security-password = Tu contraseña, para asegurarnos de que eres tú
security-key-named = Llave de seguridad «{ $name }»
security-key-added = Añadida el { $when }
security-key-actions = Acciones para { $name }
security-add-key = Añadir una llave de seguridad
key-name = Nombre
key-name-example = YubiKey azul
key-made-heading = Tu llave está creada
key-made-hint = Tu navegador la ha registrado. Dale un nombre que reconozcas. La usarás como segundo paso después de tu contraseña.
key-made-dismiss = No añadirla
key-made-done = Añadir la llave
key-added = La llave está añadida. Se cerró tu sesión en todos los demás sitios.
key-added-with-codes = La llave está añadida, y al iniciar sesión ahora se pide un segundo paso. Se cerró tu sesión en todos los demás sitios.
key-not-made = No se creó ninguna llave.
key-held-here = Esta llave ya está añadida a tu cuenta.
key-failed = Tu navegador no pudo crear una llave. Prueba con otro navegador o llave.
key-rename = Cambiar nombre
key-rename-heading = Cambiar el nombre de «{ $name }»
key-rename-confirm = Guardar
key-remove = Quitar
key-remove-heading = ¿Quitar «{ $name }»?
key-remove-hint = Ya no podrás iniciar sesión con esta llave, y se cierra tu sesión en todos los demás sitios.
key-remove-confirm = Quitar
key-removed = La llave está quitada. Se cerró tu sesión en todos los demás sitios.
security-signing-in = Inicio de sesión
security-passkey-named = Llave de acceso «{ $name }»
security-add-passkey = Añadir una llave de acceso
security-welcome = Tu cuenta está lista. Puedes añadir ahora una llave de acceso o un segundo paso, o más adelante en esta página.
security-welcome-later = Ahora no, llévame a mis proyectos
key-promote = Usar como llave de acceso
key-promote-heading = Convertir «{ $name }» en llave de acceso
key-promote-hint = La llave se configura de nuevo como llave de acceso, para que pueda iniciar tu sesión por sí sola. Tu navegador te pide tocarla y su PIN o tu huella. Después sustituye a la llave tal como está ahora. Si era tu único segundo paso, al iniciar sesión ya no se pide ninguno, tus códigos de recuperación dejan de funcionar y se cierra tu sesión en todos los demás sitios.
key-promote-confirm = Convertir en llave de acceso
key-promoted = La llave es ahora una llave de acceso e inicia tu sesión por sí sola.
key-demote = Usar como segundo paso
key-demote-heading = Usar «{ $name }» como segundo paso
key-demote-hint = La llave de acceso ya no inicia tu sesión por sí sola. Al iniciar sesión se pide primero tu contraseña y después la llave. Se cierra tu sesión en todos los demás sitios.
key-demote-confirm = Usar como segundo paso
key-demoted = La llave de acceso es ahora tu segundo paso.
key-demoted-with-codes = La llave de acceso es ahora tu segundo paso, y al iniciar sesión se pide después de tu contraseña. Se cerró tu sesión en todos los demás sitios.
two-step-unavailable = Esta instancia de Weaveling aún no admite el inicio de sesión en dos pasos. Pide a quien la opera que lo configure.
two-step-set-up-anew = Configurar de nuevo
two-step-set-up-heading = Configurar el inicio de sesión en dos pasos
two-step-set-up-hint = Después de tu contraseña, Weaveling te pedirá un código de una aplicación de tu teléfono.
two-step-scan = 1 · Escanea esto con tu aplicación de autenticación
two-step-qr = Código QR para tu aplicación de autenticación
two-step-cannot-scan = ¿No puedes escanearlo? Escribe esta clave:
two-step-copy-key = Copiar clave
two-step-code = 2 · Escribe el código que muestra tu aplicación
two-step-cancel = Cancelar
two-step-confirm = Confirmar
two-step-nothing-yet = Nada cambia hasta que se confirme este código.
two-step-on-now = El inicio de sesión en dos pasos está activado. Se cerró tu sesión en todos los demás lugares.
two-step-codes-heading = Guarda estos códigos de recuperación
two-step-codes-hint = Si pierdes tu teléfono, puedes usar cada código una vez en lugar del código de la aplicación. Solo se muestran ahora.
two-step-codes-download = Descargar como texto
two-step-codes-copy = Copiar
two-step-codes-print = Imprimir
two-step-codes-kept = Los he guardado en un lugar seguro
two-step-done = Listo
copy-done = Copiado
copy-refused = No se pudo copiar
second-step-code = El código que muestra tu aplicación para Weaveling
second-step-confirm = Confirmar
second-step-key-touch = Toca tu llave de seguridad.
second-step-key-use = Usar la llave
second-step-key-not-touched = No se tocó ninguna llave. Inténtalo otra vez o usa otra forma.
second-step-key-failed = Tu navegador no pudo pedir la llave. Usa otra forma.
second-step-another-way = Usar otra forma
second-step-first-from-now-on = Usar esta forma primero a partir de ahora
another-way-heading = Otra forma
another-way-hint = Elige cómo confirmar esta vez que eres tú.
another-way-key = Llave de seguridad
another-way-key-hint = Toca tu llave
another-way-app = Aplicación de autenticación
another-way-app-hint = Escribe el código que muestra
another-way-recovery = Código de recuperación
another-way-recovery-hint = Cada código funciona una vez
another-way-asked-first = Se pide primero ahora mismo
security-preferred-way = Pedir primero
security-preferred-key = Una llave de seguridad
security-preferred-key-hint = Tocar la llave
security-preferred-app = La aplicación de autenticación
security-preferred-app-hint = Escribir el código que muestra
security-preferred-last-used = La que usé la última vez
security-preferred-last-used-hint = La forma que usaste la última vez, en cualquier dispositivo
second-step-recovery-code = Uno de tus códigos de recuperación
second-step-recovery-hint = Cada código sirve una vez. En mayúsculas o minúsculas, con o sin guion.
second-step-no-codes = ¿No te quedan códigos ni tienes el teléfono? Quien opera esta instancia de Weaveling puede desactivar el inicio de sesión en dos pasos.
second-step-not-you = ¿no eres tú?
second-step-ran-out = El inicio de sesión tardó demasiado. Inicia sesión de nuevo.
recovery-note = Iniciaste sesión con un código de recuperación; quedan { $left }. ¿Perdiste tu teléfono?
recovery-note-last = Ese era tu último código de recuperación. Crea códigos nuevos, o vuelve a configurar el inicio de sesión en dos pasos, antes de cerrar sesión.
recovery-note-set-up-anew = Configurar de nuevo el inicio de sesión en dos pasos
two-step-draw-codes = Crear códigos nuevos
two-step-draw-codes-hint = Los códigos nuevos sustituyen a los anteriores, que dejan de funcionar.
two-step-codes-drawn = Se crearon códigos de recuperación nuevos. Los anteriores ya no funcionan.
two-step-turn-off = Desactivar
two-step-turn-off-hint = Al iniciar sesión ya no se pide un código de tu aplicación, y se cierra tu sesión en todos los demás sitios. Si no hay ninguna llave configurada como segundo paso, tus códigos de recuperación también dejan de funcionar.
two-step-set-up-password-hint = Primero escribe tu contraseña. Después verás el código que debes añadir a tu app. Configurarla de nuevo sustituye la app que usas ahora y tus códigos de recuperación.
security-add-passkey-hint = Primero escribe tu contraseña. Después tu navegador pide la llave de acceso.
security-add-key-hint = Primero escribe tu contraseña. Después tu navegador pide la llave.
security-password-go-on = Continuar
two-step-turned-off = El inicio de sesión en dos pasos está desactivado. Se cerró tu sesión en todos los demás lugares.
instance-account-two-step = Inicio de sesión en dos pasos
instance-account-ways-in = Formas de iniciar sesión
instance-account-actions = Acciones para { $name }
instance-ways-dialog = Cómo inicia sesión { $name }
instance-ways-password = Contraseña
instance-ways-password-hint = Elegida al registrarse, cambiada al restablecer la contraseña
instance-ways-always-kept = no se puede quitar
instance-ways-signs-in-alone = inicia sesión por sí sola
instance-ways-second-step = segundo paso
instance-ways-remove = Quitar
instance-ways-remove-named = Quitar { $name }
instance-ways-turn-off = Desactivar
instance-ways-turn-off-app = Desactivar la app
instance-ways-removing = Quitar «{ $name }»
instance-ways-turning-off = Desactivar la app de autenticación
instance-ways-still-password = { $name } sigue iniciando sesión con la contraseña. Se cierran todas sus sesiones.
instance-ways-still-second-step = { $name } sigue iniciando sesión con la contraseña y un segundo paso. Se cierran todas sus sesiones.
instance-ways-password-alone = { $name } iniciará sesión solo con la contraseña hasta que vuelva a configurar un segundo paso. Sus códigos de recuperación dejan de valer y se cierran todas sus sesiones.
instance-ways-make-sure = Asegúrate antes, fuera de Weaveling, de que realmente lo pide esa persona. El historial de la cuenta muestra que lo hiciste tú.
instance-ways-keep = Conservar
instance-ways-remove-passkey = Quitar la llave de acceso
instance-ways-remove-key = Quitar la llave
instance-ways-close = Cerrar
operator-heading = Administrar instancia
operator-you-are = Eres quien opera esta instancia de Weaveling.
operator-become-heading = Convertirte en operador
operator-become-hint = Introduce el código que el servidor escribió en su registro al arrancar.
operator-code = Código
operator-become-confirm = Convertirme en operador
operator-no-accounts = Esta instancia de Weaveling no tiene cuentas, así que no tiene operador.
refused-account-locked = Esta cuenta está bloqueada.
refused-not-an-operator = Solo quien opera esta instancia de Weaveling puede hacer esto.
refused-operator-cannot-be-locked = No se puede bloquear a quien opera la instancia.
refused-lock-reason-blank = Bloquear una cuenta requiere un motivo.
refused-lock-reason-control-character = Un motivo no puede contener caracteres de control.
refused-lock-reason-too-long = Un motivo puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-account-id-invalid = Ese no es un identificador de cuenta válido.
refused-account-not-found = Esa cuenta no existe.
instance-sections = Secciones
instance-accounts-heading = Cuentas
instance-accounts-more = Mostrar más
instance-account-signed-up = Registrada el { $when }
instance-account-unverified = Dirección sin confirmar
instance-invitations-heading = Invitaciones
instance-invite-email = Correo electrónico
instance-invite-confirm = Invitar
instance-invitation-withdraw = Retirar
instance-invitation-invite-again = Invitar de nuevo
instance-invitations-none = Ahora mismo no hay nadie invitado.
instance-invitations-more = Mostrar más invitaciones
instance-invitation-issued = Invitado el { $when }
instance-invitation-open = Abierta hasta el { $when }
instance-invitation-expired = Caducó el { $when }
instance-invitation-declined-address-has-account = No hace falta: este correo electrónico ya tiene una cuenta
instance-invitation-declined-turned-down = Rechazada
instance-account-operator = Opera la instancia
instance-account-locked = Bloqueada desde el { $when }
instance-account-lock = Bloquear…
instance-account-unlock = Desbloquear
instance-account-leaving = Se elimina el { $when }
instance-account-delete = Eliminar la cuenta…
instance-account-keep = Conservar esta cuenta
instance-delete-dialog = ¿Eliminar la cuenta de { $name }?
instance-delete-hint = Se cierra la sesión de { $name } en todas partes y no puede iniciarla. Su cuenta se conserva durante 30 días, hasta el { $until }, y hasta entonces puedes conservarla. Después se elimina junto con los proyectos de los que solo esa persona es miembro. Asegúrate antes, fuera de Weaveling, de que se quiere. El historial de la cuenta muestra que lo hiciste tú.
instance-delete-hand-over = { $name } es la última persona propietaria de proyectos de los que otras personas son miembros. Primero tiene que entregarlos.
instance-delete-cancel = Cancelar
instance-delete-confirm = Eliminar la cuenta
instance-lock-dialog = Bloquear a { $name }
instance-lock-hint = { $name } sale de su sesión al instante y no puede volver a entrar hasta que se desbloquee la cuenta. El motivo se guarda con el bloqueo y se muestra aquí.
instance-lock-reason = Motivo
instance-lock-cancel = Cancelar
instance-lock-confirm = Bloquear
refused-last-operator = El último operador no puede renunciar al rol, o nadie podría administrar esta instancia de Weaveling.
refused-operator-address-not-verified = Solo una cuenta con la dirección confirmada puede operar la instancia.
refused-locked-cannot-become-operator = Una cuenta bloqueada no puede operar la instancia.
refused-signing-up-by-invitation-only = En esta instancia de Weaveling solo se puede crear una cuenta por invitación.
refused-invitation-invalid = El enlace de esta invitación no es válido o ya no funciona.
refused-invitation-expired = Esta invitación ha caducado. Pide una nueva.
refused-invitation-paused = El proyecto de esta invitación está en la papelera. La invitación se podrá aceptar en cuanto se restaure el proyecto.
refused-email-address-has-account = Este correo electrónico ya tiene una cuenta. Inicia sesión.
refused-invitation-for-another-address = Esta invitación se envió a otra dirección. Inicia sesión con esa dirección para aceptarla.
refused-invitation-not-by-an-operator = En esta instancia de Weaveling solo se puede crear una cuenta con una invitación de quien la opera. Pídele que te invite.
refused-totp-unavailable = Esta instancia de Weaveling aún no admite el inicio de sesión en dos pasos. Pide a quien la opera que lo configure.
refused-totp-nothing-being-set-up = Hay que empezar de nuevo a configurar el inicio de sesión en dos pasos.
refused-totp-code-wrong = Ese no es el código que muestra tu aplicación. Espera al siguiente e inténtalo otra vez.
refused-recovery-code-wrong = Ese no es uno de tus códigos de recuperación, o ya se usó.
refused-no-second-factor = Esta cuenta no tiene inicio de sesión en dos pasos.
refused-keys-unavailable = Esta instancia de Weaveling aún no puede añadir llaves. Pide a quien la opera que fije su dirección pública.
refused-no-key-being-added = Hay que empezar de nuevo a añadir la llave.
refused-no-key-being-asked = Hay que volver a empezar a pedir la llave.
refused-key-refused = La respuesta de tu navegador no creó ninguna llave para esta instancia de Weaveling. Inténtalo otra vez.
refused-key-held-already = Esta llave ya pertenece a una cuenta.
refused-no-such-key = Esta cuenta no tiene esa llave.
refused-key-id-invalid = Ese no es un identificador de llave válido.
refused-key-name-taken = Otra de tus llaves ya tiene este nombre.
refused-key-name-blank = Una llave necesita un nombre.
refused-key-name-control-character = El nombre de una llave no puede contener caracteres de control.
refused-key-name-too-long = El nombre de una llave puede tener como máximo { $max } caracteres; este tiene { $actual }.
refused-too-many-keys = Una cuenta tiene como máximo 20 llaves. Quita una primero.
refused-password-wrong = Esa no es tu contraseña.
refused-projects-to-hand-over = Eres la última persona propietaria de proyectos de los que otras personas son miembros. Primero haz propietaria de cada uno a otra persona.
refused-no-deletion-pending = No hay ninguna eliminación pendiente para esta cuenta.
refused-deletion-asked-by-the-operator = El operador de esta instancia de Weaveling pidió eliminar esta cuenta, así que solo el operador puede cancelarlo.
refused-preferred-way-unknown = Elige la aplicación, una llave o la forma que usaste la última vez.
refused-key-role-unknown = Una llave es o bien una llave de acceso o bien un segundo paso.
instance-account-appoint = Hacer operador
instance-account-dismiss = Quitar el rol de operador
instance-appoint-dialog = Hacer de { $name } un operador
instance-appoint-hint = { $name } podrá administrar esta instancia de Weaveling como tú: listar las cuentas, bloquearlas y desbloquearlas, y nombrar operadores o quitarles el rol.
instance-appoint-confirm = Hacer operador
instance-dismiss-dialog = Quitar el rol de operador a { $name }
instance-dismiss-hint = { $name } ya no podrá administrar esta instancia de Weaveling.
instance-dismiss-yourself-hint = Ya no podrás administrar esta instancia de Weaveling, y solo otro operador podrá devolverte el rol.
instance-dismiss-confirm = Quitar el rol
instance-role-cancel = Cancelar
refused-public-url-missing = Esta instancia de Weaveling no sabe dónde se sirve, así que sus invitaciones no llevarían a ninguna parte. Pide a quien la opera que fije su dirección pública.
refused-invitation-id-invalid = Ese no es un identificador de invitación válido.
refused-invitation-not-found = Esa invitación no existe.
refused-invitation-already-accepted = Esta invitación ya fue aceptada.
refused-too-late-to-try-again = Este cambio es demasiado antiguo para volver a intentarlo.

members-link = Miembros
members-link-of = Miembros: { $names }
share = Compartir
share-title = Compartir «{ $project }»
share-people = Quién tiene acceso
share-invited = Invitaciones
share-all-members = Todos los miembros e invitaciones
members-heading = Miembros
members-hint = Quién puede leer y escribir en este proyecto.
members-you = (tú)
members-unnamed = Nombre aún desconocido
members-role-owner = Propietario/a
members-role-author = Autor/a
members-role-reader = Lector/a
roles-changed = Tu rol ha cambiado a { $role }.
members-roles = Quien es propietario/a invita, cambia los roles y borra el proyecto. Quien es autor/a lee y escribe. Quien es lector/a solo lee.
members-role-of = Rol de { $name }
members-remove = Quitar
members-leave = Salir
members-sections = Secciones
members-people = Personas
members-invited = Invitaciones
members-invite-as = Como
members-only-owner = Propietario/a, la única persona
members-last-owner-heading = Eres la única persona propietaria
members-last-owner-hint = Antes de salir, haz propietaria a otra persona. Si no, nadie podría invitar, cambiar los roles ni mover este proyecto a la papelera.
members-new-owner = Nueva persona propietaria
members-candidate = { $name } ({ $role })
members-stay = Quedarme
members-hand-over = Ceder y salir
members-alone-heading = No hay nadie más
members-alone-hint = Este proyecto no tiene otros miembros, así que si salieras, no quedaría nadie en él. Muévelo a la papelera en su lugar, donde se puede restaurar durante 30 días, o invita a alguien y cédeselo primero.
members-trash-project = Mover el proyecto a la papelera

decimal-point = { "," }
size-bytes = { $amount }{ "\u00A0" }B
size-kilobytes = { $amount }{ "\u00A0" }KB
size-megabytes = { $amount }{ "\u00A0" }MB
size-gigabytes = { $amount }{ "\u00A0" }GB
size-terabytes = { $amount }{ "\u00A0" }TB

refused-asset-id-invalid = Ese no es un identificador de imagen válido.
refused-asset-not-an-image = Este archivo no es una imagen que Weaveling pueda usar. Sube una imagen JPEG, PNG, GIF o WebP.
refused-asset-image-unreadable = No se pudo leer esta imagen. Puede que esté dañada.
refused-asset-image-too-large = Esta imagen tiene demasiados píxeles. Puede tener como máximo 50 megapíxeles.
refused-asset-limit-reached =
    { $bound ->
        [upload] Una imagen puede ocupar como máximo { $most }.
        [instance] Esta instancia de Weaveling no tiene espacio para esta imagen. Ocupa { $used } de { $most }.
        [account] No te queda espacio para esta imagen. Usas { $used } de { $most }.
        [project] Este proyecto no tiene espacio para esta imagen. Usa { $used } de { $most }.
       *[other] No queda espacio para esta imagen.
    }
refused-asset-no-account = Solo una cuenta con sesión iniciada puede tener imágenes propias.
refused-asset-limit-uncountable = Ese límite es demasiado grande.
refused-asset-sort-unknown = La lista no se puede ordenar así.
refused-asset-cursor-invalid = La lista no puede continuar desde ahí. Vuelve a cargarla.

your-account-storage = Tus archivos multimedia y el espacio que ocupan:
storage-heading = Almacenamiento
storage-used = { $used } de { $most } ocupados
storage-used-unbounded = { $used } ocupados
storage-hint = Todos los archivos multimedia que subiste, a proyectos y como propios. Borra lo que ya no necesites para liberar espacio.
storage-sort = Ordenar por
storage-sort-newest = Las más recientes primero
storage-sort-largest = Las más grandes primero
storage-none = Todavía no has subido nada.
storage-own = Tu archivo propio
storage-in-project = En { $project }
storage-in-a-project = En un proyecto
storage-detail = { $size } · subida el { $when }
storage-more = Mostrar más
storage-delete = Borrar
storage-not-deletable = Solo los autores del proyecto pueden borrarla.
storage-delete-heading = ¿Borrar este archivo?
storage-delete-hint = El archivo se borra para siempre. No se puede deshacer.
storage-delete-cancel = Cancelar
storage-delete-confirm = Borrar el archivo

size-unit-megabytes = MB
size-unit-gigabytes = GB
instance-storage-hint = Cuánto espacio ocupan los archivos multimedia en esta instancia de Weaveling.
instance-storage-accounts = Cuentas
instance-storage-find = Buscar una cuenta
instance-storage-find-hint = Nombre o correo electrónico
instance-storage-none = Ninguna cuenta ocupa espacio todavía.
instance-storage-no-match = Ninguna cuenta coincide.
instance-storage-own-limit = Límite propio
instance-storage-change-limit = Cambiar el límite
instance-storage-limit-heading = Límite de almacenamiento de { $name }
instance-storage-limit-hint = Sin un límite propio, la cuenta tiene el límite habitual para cuentas.
instance-storage-limit-amount = Límite
instance-storage-limit-unit = Unidad
instance-storage-limit-invalid = Escribe un número mayor que 0.
instance-storage-limit-cancel = Cancelar
instance-storage-limit-default = Usar el límite habitual
instance-storage-limit-save = Guardar

instance-storage-projects = Proyectos
instance-storage-find-project = Buscar un proyecto
instance-storage-find-project-hint = Nombre del proyecto
instance-storage-no-projects = Ningún proyecto ocupa espacio todavía.
instance-storage-no-project-match = Ningún proyecto coincide.
instance-storage-owners = Propietarios: { $names }
refused-picture-id-invalid = Ese no es un identificador de imagen válido.
refused-asset-in-use = Este archivo todavía está en uso, así que no se puede borrar.
storage-in-use-as-picture = Se usa como la imagen de tu cuenta.
storage-in-use = Todavía está en uso.
