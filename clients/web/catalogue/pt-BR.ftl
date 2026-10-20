masthead-all-projects = Todos os projetos
masthead-views = Visualizações
view-write = Escrever
view-board = Quadro
view-outline = Estrutura
view-ideas = Ideias
status-bar = Status

theme = Tema
theme-light = Claro
theme-system = Seguir o sistema
theme-dark = Escuro

language = Idioma
language-machine-translated = { $language } (tradução automática)

workspace-heading = Seus projetos
workspace-tagline = Traga para nós suas pequenas e frágeis ideias de histórias, e ajudaremos você a tecê-las até virarem uma grande epopeia.
loading = Carregando…
workspace-empty = Nenhum projeto ainda. Toda epopeia começa como uma pequena bagunça.

new-project-working-title = Um título provisório…
new-project-create = Criar
new-project-dialog = Novo projeto
new-project-confirm = Criar projeto

project-more-actions = Mais ações
project-rename = Renomear
project-delete = Excluir
project-rename-save = Salvar
project-rename-cancel = Cancelar

delete-project-dialog = Excluir este projeto?
delete-project-fate = “{ $name }” será apagado para sempre. A exclusão não pode ser desfeita.
delete-project-cancel = Cancelar
delete-project-confirm = Excluir

new-project-cancel = Cancelar

details-title = Título
details-book-language = Idioma do livro
details-book-language-unset = Não definido
details-book-language-hint = O idioma em que o livro é escrito, usado onde quer que o texto dependa dele.
details-quotation = Aspas
details-quotation-as-the-language-has-it = As do idioma, { $marks }
details-quotation-hint = Os sinais em volta do que é dito, colocados ao digitar ".
details-description = Descrição
details-description-placeholder = Do que trata o livro, em poucas linhas. Opcional.

book-language-de = Alemão (Deutsch)
book-language-en = Inglês (English)
book-language-fr = Francês (Français)
book-language-es = Espanhol (Español)
book-language-it = Italiano
book-language-nl = Holandês (Nederlands)
book-language-pt = Português
book-language-pl = Polonês (Polski)
book-language-sv = Sueco (Svenska)
book-language-da = Dinamarquês (Dansk)
book-language-nb = Norueguês (Norsk)
book-language-fi = Finlandês (Suomi)
book-language-cs = Tcheco (Čeština)

project-details = Detalhes do projeto
project-details-cancel = Cancelar
project-details-save = Salvar
project-details-changing-the-marks = Trocar as aspas de { $from } para { $to } as reescreve em todas as cenas do livro. Num livro longo, isso leva um tempo.
project-details-back = Voltar
project-details-change-the-marks = Trocar as aspas

restyling = Reescrevendo as aspas
restyling-waiting = Reescrevendo as aspas…
restyling-running = Reescrevendo as aspas: { $done } de { $total } { $total ->
        [one] cena
       *[other] cenas
    }.
restyling-finished = Todas as cenas agora usam { $marks }.
restyling-close = Fechar

problem-not-found =
    { $subject ->
        [appearances] Essas aparições não existem mais.
        [board] Esse quadro não existe mais.
        [idea] Essa ideia não existe mais.
        [outline] Essa estrutura não existe mais.
        [project] Esse projeto não existe mais.
        [refusal] Essa recusa não existe mais.
        [scene] Essa cena não existe mais.
        [word-counts] Essas contagens de palavras não existem mais.
       *[other] Isso não existe mais.
    }
problem-outdated =
    { $subject ->
        [appearances] Estas aparições foram alteradas em outro lugar enquanto isso. Feche isto e abra de novo para ver como estão.
        [board] Este quadro foi alterado em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
        [idea] Esta ideia foi alterada em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
        [outline] Esta estrutura foi alterada em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
        [project] Este projeto foi alterado em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
        [refusal] Esta recusa foi alterada em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
        [scene] Esta cena foi alterada em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
        [word-counts] Estas contagens de palavras foram alteradas em outro lugar enquanto isso. Feche isto e abra de novo para ver como estão.
       *[other] Isso foi alterado em outro lugar enquanto isso. Feche isto e abra de novo para ver como está.
    }
problem-overtaken = Outra pessoa alterou isso enquanto isso, então não foi possível fazer.
problem-offline = Não foi possível acessar o servidor. Tente de novo em instantes.
problem-unexpected = Algo inesperado deu errado. Tente de novo.
problem-dismiss = Dispensar

refused-project-id-invalid = Isso não é um ID de projeto válido.
refused-project-name-blank = Um projeto precisa de um título.
refused-project-name-control-character = Um título não pode conter caracteres de controle.
refused-project-name-too-long = Um título pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-book-language-not-a-tag = “{ $tag }” não é uma etiqueta de idioma como “de”, “en” ou “en-GB”.
refused-quotation-marks-unknown = “{ $name }” não são aspas que o Weaveling conheça.
refused-project-description-control-character = Uma descrição não pode conter caracteres de controle além de quebras de linha.
refused-project-description-too-long = Uma descrição pode ter no máximo { $max } caracteres; esta tem { $actual }.
refused-project-deleted = Este projeto foi excluído enquanto isso.
refused-board-id-invalid = Isso não é um ID de quadro válido.
refused-idea-already-pinned = Esta ideia já está no quadro.
refused-idea-not-pinned = Esta ideia não está no quadro.
refused-card-shapeless = Um cartão precisa de largura e altura.
refused-board-discarded = Este quadro foi descartado enquanto isso.
refused-idea-id-invalid = Isso não é um ID de ideia válido.
refused-idea-title-control-character = O título de uma ideia não pode conter caracteres de controle.
refused-idea-title-too-long = O título de uma ideia pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-idea-discarded = Esta ideia foi descartada enquanto isso.
refused-outline-id-invalid = Isso não é um ID de estrutura válido.
refused-section-title-control-character = O título de uma seção não pode conter caracteres de controle.
refused-section-title-too-long = O título de uma seção pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-outline-holds-no-such-kind = Uma estrutura contém cenas e ideias, e nada mais.
refused-section-not-found = Esta seção não está mais na estrutura.
refused-section-would-contain-itself = Uma seção não pode ser movida para dentro de si mesma.
refused-section-neighbour-not-found = Nesse lugar não há nada depois do qual colocá-lo.
refused-not-in-the-outline = Isto não está na estrutura.
refused-outline-discarded = Esta estrutura foi descartada enquanto isso.
refused-scene-id-invalid = Isso não é um ID de cena válido.
refused-scene-title-control-character = O título de uma cena não pode conter caracteres de controle.
refused-scene-title-too-long = O título de uma cena pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-quotation-never-restyled = As aspas deste livro nunca foram reescritas.

untitled = Sem título
board = Quadro
board-not-on-it = Fora do quadro · { $count }
board-waiting = Ideias fora do quadro
board-every-idea-pinned = Todas as ideias estão no quadro.
board-how-to-pin = Clique numa ideia para fixá-la onde houver espaço.
board-pin = Fixar { $idea }
board-actions = Ações para { $idea }
board-rename = Renomear { $idea }
board-open = Abrir { $idea }
board-unpin = Tirar { $idea } do quadro
idea-asked = Qual é a ideia?
board-zoom = Zoom
board-zoom-out = Diminuir o zoom
board-zoom-reset = Redefinir o zoom
board-zoom-in = Aumentar o zoom
tray = Bandeja
inspector = Inspetor
inspector-idea-name = Nome da ideia
inspector-appears-in = Aparece em
inspector-appears-nowhere = Ainda não está no livro — nenhuma seção a anota, nenhuma cena se inspira nela.
board-resize =
    { $side ->
        [top] Redimensionar { $idea } por cima
        [bottom] Redimensionar { $idea } por baixo
        [left] Redimensionar { $idea } pela esquerda
        [right] Redimensionar { $idea } pela direita
        [top-left] Redimensionar { $idea } pelo canto superior esquerdo
        [top-right] Redimensionar { $idea } pelo canto superior direito
        [bottom-left] Redimensionar { $idea } pelo canto inferior esquerdo
        [bottom-right] Redimensionar { $idea } pelo canto inferior direito
       *[other] Redimensionar { $idea }
    }
drawer-close =
    { $drawer ->
        [tray] Fechar a bandeja
        [contents] Fechar o sumário
       *[other] Fechar
    }
drawer-resize =
    { $drawer ->
        [tray] Redimensionar a bandeja
        [contents] Redimensionar o sumário
       *[other] Redimensionar
    }

scene-empty = Vazia
outline-manuscript = Manuscrito
outline-the-manuscript = O manuscrito
outline-nothing-yet = Ainda não há nada no livro. Adicione uma seção para começar.
outline-add-section = Adicionar uma seção
outline-not-in-the-book = Fora do livro · { $count }
outline-scenes = Cenas
outline-ideas = Ideias
outline-fold = Recolher { $section }
outline-section = Seção { $section }
outline-actions = Ações para { $section }
outline-move = Mover { $section }
outline-move-earlier = Adiantar { $section }
outline-move-later = Atrasar { $section }
outline-promote = Subir { $section } um nível
outline-demote = Descer { $section } um nível
outline-write-in = Escrever em { $section }
outline-remove = Remover { $section }
outline-take-out = Tirar { $name } do livro
outline-scenes-waiting = Cenas fora do livro
outline-every-scene-placed = Cada cena tem seu lugar no livro.
outline-ideas-waiting = Ideias fora do livro
outline-place = Posicionar { $name }
outline-how-to-place = Arraste para uma seção, ou clique nela e depois onde ela vai. Uma cena vira o livro; uma ideia, uma nota ao lado.
renamed-meanwhile = Renomeado enquanto isso
renamed-meanwhile-told = Enquanto você editava este título, outra pessoa o renomeou para “{ $theirs }”. O seu é “{ $mine }”.
renamed-meanwhile-theirs = Ficar com o dela
renamed-meanwhile-mine = Manter o meu

ideas = Ideias
ideas-capture = Anotar
ideas-empty = Ainda não há ideias. Lance uma ideia e veja aonde ela leva.
idea-opening = Abrindo…

book = O livro
writing-sides = Lados da obra
writing-side-book = O livro
writing-side-loose = Fora do livro
writing-every-scene-placed = Todas as cenas estão no livro.
heading-untitled = Seção sem título
heading-first = A primeira seção do seu livro
heading-next = Uma nova seção
scene-invitation = Uma cena vazia. Comece a escrever…
scene-next = Uma nova cena
scene-an-empty-one = Uma cena vazia
status-focus = Modo foco
status-focus-hint = Modo foco (Ctrl+Shift+F)
writing-settings = Configurações de escrita
count-whole-book = Contar o livro todo
count-section = Contar a seção em que você escreve
digit-group = { "." }
words =
    { $count ->
        [one] { $shown } palavra
       *[other] { $shown } palavras
    }
words-not-in-the-book = { $words } fora do livro
words-in-the-book = { $words } no livro
words-in-section = { $words } em “{ $section }”
words-in-this-section = { $words } nesta seção
saving = Salvando…
all-saved = Tudo salvo
setting-spelling = Verificar ortografia
setting-spelling-hint = Sublinha as palavras que o dicionário do navegador não conhece.
setting-justified = Texto justificado
setting-justified-hint = Alinha as linhas dos dois lados como uma página impressa, hifenizando as palavras no idioma do livro.
setting-typewriter = Rolagem de máquina de escrever
setting-typewriter-hint = Mantém a linha em que você escreve no meio da janela.
settings-close = Fechar
contents = Sumário
this-scene = Esta cena
where-you-are = Onde você está
quiet-rearranged = Outra pessoa reorganizou o livro em torno de “{ $section }”.
quiet-removed = Outra pessoa removeu “{ $section }”.
quiet-overtaken = Outra pessoa alterou o livro enquanto isso, então não foi possível fazer. Ele mostra o livro como está agora.
going-back = Voltar para onde você parou
