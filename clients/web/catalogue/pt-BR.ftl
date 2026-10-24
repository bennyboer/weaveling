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
theme-chosen = Tema: { $theme }

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
        [account] Essa conta não existe mais.
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

move-scene = Mover a cena
drag-to-move = Arraste para mover
scene-actions = Ações da cena
menu-rename = Renomear
menu-take-out = Tirar do livro
menu-bring-in = Levar para o livro…
menu-delete-scene = Excluir a cena
section-actions = Ações da seção
menu-nest = Aninhar sob a seção anterior
menu-lift = Subir um nível
menu-remove-section = Remover a seção
add-here = Adicionar aqui
menu-new-scene = Nova cena
menu-new-section = Nova seção
menu-new-top-level-section = Nova seção de primeiro nível
menu-bring-back = Trazer de volta uma cena…
scene-title = Título da cena
menu-button-title = { $menu } ({ $keys })
keys-tab = Tab
keys-shift-tab = Shift+Tab
keys-shift-f10 = Shift+F10
cancel = Cancelar
asked-keep-writing = Continuar escrevendo
answer-delete = Excluir
answer-remove = Remover
answer-show-it = Mostrar
answer-discard = Descartar
answer-put-at-end = Colocar no fim do livro
answer-take-out = Tirar do livro
answer-delete-for-good = Excluir para sempre
asked-delete-scene = Excluir esta cena?
asked-delete-scene-told-unnamed = Esta cena e tudo o que está escrito nela serão apagados para sempre. A exclusão não pode ser desfeita.
asked-delete-scene-told = “{ $scene }” e tudo o que está escrito nela serão apagados para sempre. A exclusão não pode ser desfeita.
removed-meanwhile = Removida enquanto isso
removed-meanwhile-told-unnamed = Outra pessoa removeu “{ $section }”. Sua cena agora está fora do livro, entre as cenas que não estão nele.
removed-meanwhile-told = Outra pessoa removeu “{ $section }”. Sua cena “{ $scene }” agora está fora do livro, entre as cenas que não estão nele.
place-gone = O lugar dela sumiu
place-gone-scene = Outra pessoa removeu o lugar para onde ia sua nova cena. Ela começa com “{ $begins }”.
place-gone-section = Outra pessoa removeu aquilo depois do qual sua nova seção “{ $section }” ia ficar.
renamed-meanwhile-heading-told = Enquanto você editava este título, outra pessoa o renomeou para “{ $theirs }”. O seu é “{ $mine }”.
asked-remove-section = Remover esta seção?
asked-remove-section-told = As cenas dela saem do livro, mas não são excluídas: esperam entre as cenas que não estão no livro. As seções dentro dela sobem para ocupar seu lugar.
asked-remove-chosen = Remover o que foi escolhido?
chosen-scenes =
    { $scenes ->
        [one] { $scenes } cena escolhida.
       *[other] { $scenes } cenas escolhidas.
    }
chosen-sections =
    { $sections ->
        [one] { $sections } seção escolhida.
       *[other] { $sections } seções escolhidas.
    }
chosen-both =
    { $scenes ->
        [one] { $scenes } cena
       *[other] { $scenes } cenas
    } e { $sections ->
        [one] { $sections } seção
       *[other] { $sections } seções
    } escolhidas.
chosen-whole = Cada seção vai junto com as seções dentro dela e todas as suas cenas.
chosen-ways-book = Tiradas do livro, as cenas esperam entre as que não estão nele. Excluídas para sempre, elas e tudo o que está escrito nelas somem, e isso não pode ser desfeito.
chosen-ways-loose = Excluídas para sempre, elas e tudo o que está escrito nelas somem, e isso não pode ser desfeito.
placing = Levar para o livro
placing-told-unnamed = A cena vai para o fim da seção que você escolher.
placing-told = “{ $scene }” vai para o fim da seção que você escolher.
placing-sections = Seções do livro
bring-back = Trazer de volta uma cena
looking = Procurando…
bring-back-none = Todas as cenas já estão no livro.
bring-back-told = Estas cenas não estão no livro. A que você escolher entra onde você clicou.
bring-back-find = Procurar uma cena
bring-back-no-match = Nenhuma cena corresponde.

contents-unfold-all = Expandir tudo
contents-fold-all = Recolher tudo
contents-unfold = Expandir { $section }
this-section = Esta seção
inspector-nothing-yet = Coloque o cursor numa cena ou num título para vê-lo aqui.
inspector-section-removed = Esta seção foi removida.
pinned-ideas = Ideias anotadas
pinned-none = Nenhuma ideia anotada aqui.
pin-an-idea = Anotar uma ideia
section-unpin = Tirar { $idea }
drawn-from = Inspira-se em
drawn-from-none = Nenhuma ideia vinculada ainda.
link-an-idea = Vincular uma ideia
unlink-idea = Desvincular { $idea }
pin-ideas = Anotar ideias
ideas-to-pin = Ideias para anotar
every-idea-pinned = Todas as ideias deste projeto já estão anotadas na estrutura.
pin-ideas-confirm =
    { $count ->
        [0] Anotar
        [1] Anotar
       *[other] Anotar { $count }
    }
link-ideas = Vincular ideias
ideas-to-link = Ideias para vincular
every-idea-linked = Todas as ideias deste projeto já estão vinculadas.
link-ideas-confirm =
    { $count ->
        [0] Vincular
        [1] Vincular
       *[other] Vincular { $count }
    }
find-an-idea = Procurar uma ideia
no-idea-matches = Nenhuma ideia corresponde.

refusals = Alterações que não puderam ser aplicadas
refusals-told = Foram salvas, mas não chegaram a todos os lugares aonde deviam. Tente de novo quando o que as impediu for resolvido, ou confirme-as para silenciar o alarme.
refusals-none = Não há mais nada para ver.
refusals-list = Alterações recusadas
close = Fechar
refusal-wordless = Uma alteração não chegou a todos os lugares aonde devia.
refusal-given-up = Abandonada em { $when }
refusal-details = Detalhes
refusal-listener = Receptor
refusal-message = Mensagem
refusal-attempts = Tentativas
refusal-reason = Motivo
refusal-retry = Tentar de novo
refusal-acknowledge = Confirmar
alarm-note = Algumas alterações não puderam ser aplicadas.
alarm-show = Mostrar
missing = Não há nada tecido neste endereço.
missing-back = Voltar para seus projetos

moment =
    { $day } de { $month ->
        [1] jan.
        [2] fev.
        [3] mar.
        [4] abr.
        [5] mai.
        [6] jun.
        [7] jul.
        [8] ago.
        [9] set.
        [10] out.
        [11] nov.
       *[12] dez.
    } de { $year }, { $hour }:{ $minute }

sign-in-heading = Entrar
sign-in-email = E-mail
sign-in-password = Senha
sign-in-confirm = Entrar
sign-out = Sair
refused-email-malformed = Isso não é um e-mail como nome@exemplo.com.
refused-email-too-long = Um e-mail pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-account-name-blank = Uma conta precisa de um nome.
refused-account-name-control-character = Um nome não pode conter caracteres de controle.
refused-account-name-too-long = Um nome pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-password-too-short = Uma senha precisa de pelo menos { $min } caracteres; esta tem { $actual }.
refused-password-too-long = Uma senha pode ter no máximo { $max } caracteres; esta tem { $actual }.
refused-verification-code-wrong = Esse não é o código que enviamos.
refused-verification-code-expired = Este código expirou. Peça um novo.
refused-verification-code-exhausted = Este código foi tentado vezes demais. Peça um novo.
refused-sign-in-refused = Com este e-mail e esta senha não é possível entrar.
refused-address-not-verified = Este e-mail ainda não foi confirmado. Digite o código que enviamos para ele.
refused-not-signed-in = Entre primeiro.
sign-up-heading = Criar conta
sign-up-name = Seu nome
sign-up-password-hint = Pelo menos 12 caracteres.
sign-up-confirm = Criar conta
sign-up-instead = Ainda não tem conta? Crie uma
sign-in-instead = Já tem conta? Entre
code-heading = Confirme seu e-mail
code-sent = Enviamos um código para { $email }.
code-field = Código
code-confirm = Confirmar
code-again = Enviar um código novo
code-sent-again = Um código novo está a caminho.
code-confirmed = Seu e-mail está confirmado. Você já pode entrar.
code-enter = Digitar o código
unreachable-heading = Não foi possível acessar o servidor
try-again = Tentar de novo
account-menu = Conta: { $name }
reset-instead = Esqueceu a senha?
reset-heading = Criar uma senha nova
reset-ask = Enviar um código
reset-password = Senha nova
reset-confirm = Salvar a senha nova
reset-done = Sua senha nova está salva. Você já pode entrar com ela.
back-to-sign-in = Voltar para entrar
refused-too-many-tries =
    { $minutes ->
        [one] Tentativas demais. Espere um minuto antes de tentar de novo.
       *[other] Tentativas demais. Espere { $minutes } minutos antes de tentar de novo.
    }
refused-operator-code-wrong = Este não é o código que o servidor escreveu no log.
account-menu-operator = Gerenciar instância
operator-heading = Gerenciar instância
operator-you-are = Você é quem opera esta instância do Weaveling.
operator-become-heading = Tornar-se o operador
operator-become-hint = Digite o código que o servidor escreveu no log ao iniciar.
operator-code = Código
operator-become-confirm = Tornar-me o operador
operator-no-accounts = Esta instância do Weaveling não guarda contas, então não tem operador.
refused-account-locked = Esta conta está bloqueada.
refused-not-an-operator = Só quem opera esta instância do Weaveling pode fazer isso.
refused-operator-cannot-be-locked = Quem opera a instância não pode ser bloqueado.
refused-lock-reason-blank = Bloquear uma conta exige um motivo.
refused-lock-reason-control-character = Um motivo não pode conter caracteres de controle.
refused-lock-reason-too-long = Um motivo pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-account-id-invalid = Este não é o identificador de uma conta.
refused-account-not-found = Essa conta não existe.
instance-sections = Seções
instance-accounts-heading = Contas
instance-accounts-more = Mostrar mais
instance-account-signed-up = Cadastrada em { $when }
instance-account-unverified = Endereço não confirmado
instance-invitations-heading = Convites
instance-invitations-none = Ninguém está convidado no momento.
instance-invitations-more = Mostrar mais convites
instance-invitation-issued = Convidado em { $when }
instance-invitation-open = Aberto até { $when }
instance-invitation-expired = Expirou em { $when }
instance-invitation-declined-address-has-account = Não é preciso: este e-mail já tem uma conta
instance-account-operator = Opera a instância
instance-account-locked = Bloqueada desde { $when }
instance-account-lock = Bloquear…
instance-account-unlock = Desbloquear
instance-lock-dialog = Bloquear { $name }
instance-lock-hint = { $name } sai da sessão na hora e não pode entrar de novo até ser desbloqueada. O motivo fica guardado com o bloqueio e aparece aqui.
instance-lock-reason = Motivo
instance-lock-cancel = Cancelar
instance-lock-confirm = Bloquear
refused-last-operator = Quem opera a instância por último mantém o papel, senão ninguém poderia gerenciar esta instância do Weaveling.
refused-operator-address-not-verified = Só uma conta com endereço confirmado pode operar a instância.
refused-locked-cannot-become-operator = Uma conta bloqueada não pode operar a instância.
refused-signing-up-by-invitation-only = Nesta instância do Weaveling só é possível criar uma conta por convite.
refused-invitation-invalid = O link deste convite não permite criar nenhuma conta, ou não mais.
refused-invitation-expired = Este convite expirou. Peça um novo.
refused-email-address-has-account = Este e-mail já tem uma conta. Entre com ela.
instance-account-appoint = Tornar operador
instance-account-dismiss = Tirar o papel de operador
instance-appoint-dialog = Tornar { $name } operador
instance-appoint-hint = { $name } poderá então gerenciar esta instância do Weaveling como você: listar as contas, bloqueá-las e desbloqueá-las, e dar ou tirar o papel de operador.
instance-appoint-confirm = Tornar operador
instance-dismiss-dialog = Tirar o papel de operador de { $name }
instance-dismiss-hint = { $name } não poderá mais gerenciar esta instância do Weaveling.
instance-dismiss-yourself-hint = Você não poderá mais gerenciar esta instância do Weaveling, e só outro operador poderá devolver o papel.
instance-dismiss-confirm = Tirar o papel
instance-role-cancel = Cancelar
refused-origin-missing = O link de um convite é feito a partir da página de onde ele é enviado, e esta requisição não nomeou nenhuma.
refused-invitation-id-invalid = Este não é o identificador de um convite.
refused-invitation-not-found = Esse convite não existe.
refused-invitation-already-accepted = Este convite já foi aceito.
