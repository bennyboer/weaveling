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
language-machine-translated = { $language } (traducción automática)

workspace-heading = Tus proyectos
workspace-tagline = Tráenos tus pequeñas y frágiles ideas para historias, y te ayudaremos a tejerlas hasta formar una gran epopeya.
workspace-loading = Cargando…
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
refused-project-not-started-yet = Este proyecto aún no se ha empezado.
refused-project-already-started = Este proyecto ya se ha empezado.
refused-project-deleted = Este proyecto se eliminó mientras tanto.
