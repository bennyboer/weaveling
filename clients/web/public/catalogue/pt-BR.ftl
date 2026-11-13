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
masthead-more = Mais
masthead-this-project = Este projeto
language-machine-translated = { $language } (tradução automática)

workspace-heading = Seus projetos
workspace-tagline = Traga para nós suas pequenas e frágeis ideias de histórias, e ajudaremos você a tecê-las até virarem uma grande epopeia.
loading = Carregando…
workspace-empty = Nenhum projeto ainda. Toda epopeia começa como uma pequena bagunça.
workspace-trash-link = Lixeira, { $count } { $count ->
        [one] projeto
       *[other] projetos
    }

new-project-working-title = Um título provisório…
new-project-create = Criar
new-project-dialog = Novo projeto
new-project-confirm = Criar projeto
new-project-setting-up = Preparando seu projeto…

project-more-actions = Mais ações
project-rename = Renomear
project-trash = Mover para a lixeira
project-rename-save = Salvar
project-rename-cancel = Cancelar

trash-project-dialog = Mover este projeto para a lixeira?
trash-project-fate = “{ $name }” pode ser restaurado da lixeira por 30 dias. Depois disso, será apagado para sempre.
trash-project-cancel = Cancelar
trash-project-confirm = Mover para a lixeira
trash-heading = Lixeira
trash-empty = A lixeira está vazia.
trash-kept-until = Pode ser restaurado até { $moment }
trash-restore = Restaurar

new-project-cancel = Cancelar

details-title = Título
details-book-language = Idioma do livro
details-book-language-unset = Não definido
details-book-language-hint = O idioma em que seu livro é escrito. A hifenização e as aspas seguem esse idioma.
details-quotation = Aspas
details-quotation-as-the-language-has-it = O padrão do idioma, { $marks }
details-quotation-hint = Os sinais colocados em volta dos diálogos quando você digita ".
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
        [appearances] Estas aparições foram alteradas por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [board] Este quadro foi alterado por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [idea] Esta ideia foi alterada por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [outline] Esta estrutura foi alterada por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [project] Este projeto foi alterado por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [refusal] Esta recusa foi alterada por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [scene] Esta cena foi alterada por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
        [word-counts] Estas contagens de palavras foram alteradas por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
       *[other] Isso foi alterado por outra pessoa nesse meio-tempo. Feche isto e abra de novo para ver a versão mais recente.
    }
problem-overtaken = Outra pessoa alterou isso nesse meio-tempo, então não foi possível fazer.
problem-offline = Não foi possível acessar o servidor. Tente de novo em instantes.
problem-unexpected = Algo inesperado deu errado. Tente de novo.
problem-dismiss = Dispensar

refused-project-id-invalid = Isso não é um ID de projeto válido.
refused-project-name-blank = Um projeto precisa de um título.
refused-project-name-control-character = Um título não pode conter caracteres de controle.
refused-project-name-too-long = Um título pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-project-not-yet-open = Seu projeto foi criado, mas ainda não pode ser aberto. Ele vai aparecer na sua lista em instantes.
refused-book-language-not-a-tag = “{ $tag }” não é uma etiqueta de idioma como “de”, “en” ou “en-GB”.
refused-quotation-marks-unknown = O Weaveling não conhece as aspas “{ $name }”.
refused-project-description-control-character = Uma descrição não pode conter caracteres de controle além de quebras de linha.
refused-project-description-too-long = Uma descrição pode ter no máximo { $max } caracteres; esta tem { $actual }.
refused-project-deleted = Este projeto foi excluído nesse meio-tempo.
refused-project-trashed = Este projeto foi movido para a lixeira nesse meio-tempo.
refused-project-not-trashed = Este projeto não está na lixeira.
refused-not-an-owner = Só quem é dono deste projeto pode fazer isso.
refused-last-owner = Um projeto mantém pelo menos uma pessoa como dona. Torne outra pessoa dona primeiro.
refused-not-a-member = Essa pessoa não é membro deste projeto.
refused-role-unknown = Esse papel não existe em um projeto.
refused-member-leaving = Esta conta está prestes a ser excluída, então o papel dela não pode ser alterado.
refused-not-an-author = Só quem escreve neste projeto pode fazer isso.
refused-board-id-invalid = Isso não é um ID de quadro válido.
refused-idea-already-pinned = Esta ideia já está no quadro.
refused-idea-not-pinned = Esta ideia não está no quadro.
refused-card-shapeless = Um cartão precisa de largura e altura.
refused-board-discarded = Este quadro foi descartado nesse meio-tempo.
refused-idea-id-invalid = Isso não é um ID de ideia válido.
refused-idea-title-control-character = O título de uma ideia não pode conter caracteres de controle.
refused-idea-title-too-long = O título de uma ideia pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-idea-discarded = Esta ideia foi descartada nesse meio-tempo.
refused-outline-id-invalid = Isso não é um ID de estrutura válido.
refused-section-title-control-character = O título de uma seção não pode conter caracteres de controle.
refused-section-title-too-long = O título de uma seção pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-outline-holds-no-such-kind = Uma estrutura contém cenas e ideias, e nada mais.
refused-section-not-found = Esta seção não está mais na estrutura.
refused-section-would-contain-itself = Uma seção não pode ser movida para dentro de si mesma.
refused-section-neighbour-not-found = O item depois do qual isto deveria ficar não existe mais.
refused-not-in-the-outline = Isto não está na estrutura.
refused-outline-discarded = Esta estrutura foi descartada nesse meio-tempo.
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
inspector-appears-nowhere = Ainda não está no livro. Nenhuma seção ou cena a usa.
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
outline-how-to-place = Arraste-a para uma seção, ou clique nela e depois clique onde ela deve ficar. Uma cena passa a fazer parte do livro. Uma ideia vira uma nota ao lado.
renamed-meanwhile = Renomeado nesse meio-tempo
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
path = Caminho
path-home = Início
path-invitation = Convite
path-missing = Não encontrado
quiet-rearranged = Outra pessoa reorganizou o livro em torno de “{ $section }”.
quiet-removed = Outra pessoa removeu “{ $section }”.
quiet-overtaken = Outra pessoa alterou o livro nesse meio-tempo, então não foi possível fazer. Agora você vê a versão atual.
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
removed-meanwhile = Removida nesse meio-tempo
removed-meanwhile-told-unnamed = Outra pessoa removeu “{ $section }”. Sua cena agora está entre as cenas que não estão no livro.
removed-meanwhile-told = Outra pessoa removeu “{ $section }”. Sua cena “{ $scene }” agora está entre as cenas que não estão no livro.
place-gone = O lugar dela sumiu
place-gone-scene = Outra pessoa removeu o lugar para onde ia sua nova cena. Ela começa com “{ $begins }”.
place-gone-section = Outra pessoa removeu o item depois do qual sua nova seção “{ $section }” ia ficar.
renamed-meanwhile-heading-told = Enquanto você editava este título, outra pessoa o renomeou para “{ $theirs }”. O seu é “{ $mine }”.
asked-remove-section = Remover esta seção?
asked-remove-section-told = As cenas dela saem do livro, mas não são excluídas. Você vai encontrá-las entre as cenas que não estão no livro. As seções dentro dela sobem um nível.
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
chosen-whole = Cada seção é removida junto com as seções dentro dela e todas as suas cenas.
chosen-ways-book = Se você tirá-las do livro, as cenas vão para as cenas que não estão no livro. Se você excluí-las, elas e tudo o que está escrito nelas somem de vez. Isso não pode ser desfeito.
chosen-ways-loose = Se você excluí-las, elas e tudo o que está escrito nelas somem de vez. Isso não pode ser desfeito.
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
refusals-told = Estas alterações foram salvas, mas nem todas as partes do Weaveling as receberam. Corrija a causa e tente de novo, ou confirme-as para silenciar o alarme.
refusals-none = Não há mais nada para ver.
refusals-list = Alterações recusadas
close = Fechar
refusal-wordless = Uma alteração não chegou a todas as partes do Weaveling.
refusal-given-up = Abandonada em { $when }
refusal-details = Detalhes
refusal-listener = Receptor
refusal-message = Mensagem
refusal-attempts = Tentativas
refusal-reason = Motivo
refusal-retry = Tentar de novo
refusal-too-old = Esta alteração tem mais de 30 dias e não é tentada de novo, porque aquilo de que tratava pode não existir mais. Confirme-a para silenciar o alarme.
refusal-acknowledge = Confirmar
alarm-note = Algumas alterações não puderam ser aplicadas.
alarm-show = Mostrar
missing = Não há nada tecido neste endereço.

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

day =
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
    } de { $year }

deletion-pending-heading = Sua conta será excluída
deletion-pending-asked = Você pediu isso em { $asked }. Em { $until }, sua conta e os projetos dos quais só você é membro serão excluídos de vez.
deletion-pending-nothing-lost = Até lá, nada se perde. Se você mantiver sua conta, continua conectado e encontra cada projeto como o deixou.
deletion-pending-keep = Manter minha conta
deletion-pending-let-go = Sair e deixar que seja excluída
deletion-pending-by-the-operator = O operador desta instância do Weaveling agendou a exclusão da sua conta para { $until }. Fale com ele se quiser mantê-la.
deletion-pending-sign-out = Sair
your-account-heading = Sua conta
your-account-security = Como você entra:
your-account-delete-heading = Excluir esta conta
your-account-delete-hint = Sua conta e tudo o que pertence a ela são excluídos de vez após 30 dias.
your-account-delete = Excluir esta conta…
your-account-delete-question = Excluir sua conta?
your-account-delete-kept = Você sai de todos os lugares na hora. Sua conta e seu trabalho são mantidos por 30 dias, até { $until }. Entre antes disso para manter tudo. Depois disso, tudo some de vez.
your-account-delete-alone = Os projetos dos quais só você é membro também são excluídos.
your-account-delete-password = Sua senha, para termos certeza de que é você
your-account-keep = Manter minha conta
your-account-delete-confirm = Excluir minha conta
your-account-hand-over-heading = Passe estes projetos adiante primeiro
your-account-hand-over-hint = Você é a única pessoa dona de projetos dos quais outras pessoas também são membros. Torne uma delas dona, ou exclua o projeto. Depois você poderá excluir sua conta.
your-account-hand-over-members = Membros
your-account-hand-over-close = Fechar

sign-in-heading = Entrar
sign-in-email = E-mail
sign-in-password = Senha
sign-in-confirm = Entrar
passkey-sign-in = Entrar com uma chave de acesso
passkey-or-password = ou com sua senha
passkey-not-touched = Nenhuma chave de acesso foi usada. Tente de novo ou entre com sua senha.
passkey-failed = Seu navegador não conseguiu pedir uma chave de acesso. Entre com sua senha.
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
refused-sign-in-refused = O e-mail ou a senha estão incorretos.
refused-address-not-verified = Este e-mail ainda não foi confirmado. Digite o código que enviamos para ele.
refused-not-signed-in = Entre primeiro.
sign-up-heading = Criar conta
sign-up-name = Seu nome
sign-up-password-hint = Pelo menos 12 caracteres.
sign-up-confirm = Criar conta
sign-up-instead = Ainda não tem conta? Crie uma
sign-in-instead = Já tem conta? Entre
set-up-instead = Configurar esta instância do Weaveling
set-up-heading = Configurar esta instância do Weaveling
set-up-hint = Esta instância do Weaveling ainda não tem operador. Crie uma conta com o código do log do servidor. Assim que seu endereço for confirmado, você passa a ser o operador dela.
set-up-code = Código do log do servidor
set-up-done = Esta instância do Weaveling já está configurada e tem um operador. Um operador pode tornar você operador também.
invitation-accept-heading = Você recebeu um convite
invitation-accept-hint = Escolha seu nome e uma senha. Sua conta usará o e-mail para o qual o convite foi enviado.
invitation-signed-in = Você entrou como { $email }. Aceite o convite para participar do projeto.
invitation-accept = Aceitar
invitation-accepted = Convite aceito. O projeto vai aparecer na sua lista em instantes.
invitation-decline = Recusar
invitation-declined = Você recusou o convite.
invitation-decline-instead = Recusar o convite
invitation-declined-heading = Convite recusado
invitation-declined-hint = Você o recusou. Quem convidou você verá isso, e o link não funciona mais.
notifications = Notificações
notifications-waiting = Notificações: { $count } novas
notifications-none = Nada de novo agora.
notifications-invitations = Convites
notice-invitation-from = De { $by }
notice-invitation-unnamed-project = um projeto
notice-invitation-join = Entrar
notice-invitation-decline = Recusar
notice-invitation-joined = Você entrou em { $project }. Ele vai aparecer na sua lista em instantes.
notice-invitation-declined = Você recusou o convite para { $project }.
notifications-roles = Papéis
notice-role-unnamed-project = um projeto
notice-role-changed-by = { $by } mudou seu papel em { $project } para { $role ->
        [owner] dono(a)
        [author] autor(a)
       *[reader] leitor(a)
    }.
notice-role-changed = Seu papel em { $project } agora é { $role ->
        [owner] dono(a)
        [author] autor(a)
       *[reader] leitor(a)
    }.
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
path-account = Sua conta
security-heading = Segurança
security-hint = Como você entra nesta instância do Weaveling.
security-second-step = Segunda etapa
security-second-step-hint = Depois da sua senha, o Weaveling pode pedir uma segunda etapa, para que só a senha não baste para entrar.
security-app = App autenticador
security-app-on-since = Ativado desde { $when }
security-app-actions = Ações para o app autenticador
security-codes = Códigos de recuperação
security-codes-left = Restam { $left } de 10
security-set-up-app = Configurar um app autenticador
security-password = Sua senha, para ter certeza de que é você
security-key-named = Chave de segurança “{ $name }”
security-key-added = Adicionada em { $when }
security-key-actions = Ações para { $name }
security-add-key = Adicionar uma chave de segurança
key-name = Nome
key-name-example = YubiKey azul
key-made-heading = Sua chave foi criada
key-made-hint = Seu navegador a registrou. Dê a ela um nome que você reconheça. Você vai usá-la como segunda etapa depois da sua senha.
key-made-dismiss = Não adicionar
key-made-done = Adicionar a chave
key-added = A chave foi adicionada. Você saiu em todos os outros lugares.
key-added-with-codes = A chave foi adicionada, e entrar agora pede uma segunda etapa. Você saiu em todos os outros lugares.
key-not-made = Nenhuma chave foi criada.
key-held-here = Esta chave já está adicionada à sua conta.
key-failed = Seu navegador não conseguiu criar uma chave. Tente outro navegador ou outra chave.
key-rename = Renomear
key-rename-heading = Renomear “{ $name }”
key-rename-confirm = Salvar
key-remove = Remover
key-remove-heading = Remover “{ $name }”?
key-remove-hint = Você não pode mais entrar com esta chave e sai em todos os outros lugares.
key-remove-confirm = Remover
key-removed = A chave foi removida. Você saiu em todos os outros lugares.
security-signing-in = Entrada
security-passkey-named = Chave de acesso “{ $name }”
security-add-passkey = Adicionar uma chave de acesso
security-welcome = Sua conta está pronta. Uma chave de acesso ou um segundo passo pode ser adicionado agora, ou a qualquer momento depois nesta página.
security-welcome-later = Agora não, me leve aos meus projetos
key-promote = Usar como chave de acesso
key-promote-heading = Tornar “{ $name }” uma chave de acesso
key-promote-hint = A chave é configurada de novo como chave de acesso, para poder fazer você entrar sozinha. Seu navegador pede que você a toque, e o PIN dela ou sua digital. Depois ela substitui a chave como está agora. Se era sua única segunda etapa, entrar não pede mais nenhuma, seus códigos de recuperação deixam de funcionar e você sai em todos os outros lugares.
key-promote-confirm = Tornar chave de acesso
key-promoted = A chave agora é uma chave de acesso e faz você entrar sozinha.
key-demote = Usar como segunda etapa
key-demote-heading = Usar “{ $name }” como segunda etapa
key-demote-hint = A chave de acesso não faz mais você entrar sozinha. Ao entrar, primeiro é pedida sua senha, depois a chave. Você sai em todos os outros lugares.
key-demote-confirm = Usar como segunda etapa
key-demoted = A chave de acesso agora é sua segunda etapa.
key-demoted-with-codes = A chave de acesso agora é sua segunda etapa, e ao entrar ela é pedida depois da sua senha. Você saiu em todos os outros lugares.
two-step-unavailable = Esta instância do Weaveling ainda não oferece o login em duas etapas. Peça a quem a opera que o configure.
two-step-set-up-anew = Configurar de novo
two-step-set-up-heading = Configurar o login em duas etapas
two-step-set-up-hint = Depois da sua senha, o Weaveling vai pedir um código de um app no seu celular.
two-step-scan = 1 · Escaneie isto com seu app autenticador
two-step-qr = Código QR para seu app autenticador
two-step-cannot-scan = Não consegue escanear? Digite esta chave:
two-step-copy-key = Copiar chave
two-step-code = 2 · Digite o código que seu app mostra
two-step-cancel = Cancelar
two-step-confirm = Confirmar
two-step-nothing-yet = Nada muda até que este código seja confirmado.
two-step-on-now = O login em duas etapas está ativado. Você saiu de todos os outros lugares.
two-step-codes-heading = Guarde estes códigos de recuperação
two-step-codes-hint = Se você perder seu celular, pode usar cada código uma vez no lugar do código do app. Eles só são mostrados agora.
two-step-codes-download = Baixar como texto
two-step-codes-copy = Copiar
two-step-codes-print = Imprimir
two-step-codes-kept = Eu os guardei em um lugar seguro
two-step-done = Concluir
copy-done = Copiado
copy-refused = Não foi possível copiar
second-step-code = O código que seu app mostra para o Weaveling
second-step-confirm = Confirmar
second-step-key-touch = Toque na sua chave de segurança.
second-step-key-use = Usar a chave
second-step-key-not-touched = Nenhuma chave foi tocada. Tente de novo ou use outro jeito.
second-step-key-failed = Seu navegador não conseguiu pedir a chave. Use outro jeito.
second-step-another-way = Usar outro jeito
second-step-first-from-now-on = Usar este jeito primeiro de agora em diante
another-way-heading = Outro jeito
another-way-hint = Escolha como confirmar que é você desta vez.
another-way-key = Chave de segurança
another-way-key-hint = Toque na sua chave
another-way-app = App autenticador
another-way-app-hint = Digite o código que ele mostra
another-way-recovery = Código de recuperação
another-way-recovery-hint = Cada código funciona uma vez
another-way-asked-first = Pedido primeiro agora
security-preferred-way = Pedir primeiro
security-preferred-key = Uma chave de segurança
security-preferred-key-hint = Tocar na chave
security-preferred-app = O app autenticador
security-preferred-app-hint = Digitar o código que ele mostra
security-preferred-last-used = O que usei por último
security-preferred-last-used-hint = A forma que você usou da última vez, em qualquer dispositivo
second-step-recovery-code = Um dos seus códigos de recuperação
second-step-recovery-hint = Cada código funciona uma vez. Maiúsculas ou minúsculas, com ou sem hífen.
second-step-no-codes = Sem códigos e sem celular? Quem opera esta instância do Weaveling pode desativar o login em duas etapas para você.
second-step-not-you = não é você?
second-step-ran-out = O login demorou demais. Entre de novo.
recovery-note = Você entrou com um código de recuperação; restam { $left }. Perdeu o celular?
recovery-note-last = Esse era seu último código de recuperação. Crie novos, ou configure de novo o login em duas etapas, antes de sair.
recovery-note-set-up-anew = Configurar de novo o login em duas etapas
two-step-draw-codes = Criar códigos novos
two-step-draw-codes-hint = Os códigos novos substituem os antigos, que deixam de funcionar.
two-step-codes-drawn = Novos códigos de recuperação foram criados. Os antigos não funcionam mais.
two-step-turn-off = Desativar
two-step-turn-off-hint = Entrar não pede mais um código do seu app, e você sai em todos os outros lugares. Se nenhuma chave estiver configurada como segunda etapa, seus códigos de recuperação também deixam de funcionar.
two-step-set-up-password-hint = Digite primeiro sua senha. Depois você vê o código para adicionar ao seu app. Configurar de novo substitui o app que você usa agora e seus códigos de recuperação.
security-add-passkey-hint = Digite primeiro sua senha. Depois seu navegador pede a chave de acesso.
security-add-key-hint = Digite primeiro sua senha. Depois seu navegador pede a chave.
security-password-go-on = Continuar
two-step-turned-off = O login em duas etapas está desativado. Você saiu de todos os outros lugares.
instance-account-two-step = Login em duas etapas
instance-account-ways-in = Formas de entrar
instance-account-actions = Ações para { $name }
instance-ways-dialog = Como { $name } entra
instance-ways-password = Senha
instance-ways-password-hint = Definida no cadastro, alterada por uma redefinição de senha
instance-ways-always-kept = não pode ser removida
instance-ways-signs-in-alone = entra sozinha
instance-ways-second-step = segundo passo
instance-ways-remove = Remover
instance-ways-remove-named = Remover { $name }
instance-ways-turn-off = Desativar
instance-ways-turn-off-app = Desativar o app
instance-ways-removing = Remover “{ $name }”
instance-ways-turning-off = Desativar o app autenticador
instance-ways-still-password = { $name } continua entrando com a senha. Todas as sessões são encerradas.
instance-ways-still-second-step = { $name } continua entrando com a senha e um segundo passo. Todas as sessões são encerradas.
instance-ways-password-alone = { $name } vai entrar só com a senha até configurar um segundo passo de novo. Os códigos de recuperação deixam de valer e todas as sessões são encerradas.
instance-ways-make-sure = Antes, confirme fora do Weaveling que é mesmo essa pessoa pedindo. O histórico da conta mostra que foi você.
instance-ways-keep = Manter
instance-ways-remove-passkey = Remover a chave de acesso
instance-ways-remove-key = Remover a chave
instance-ways-close = Fechar
operator-heading = Gerenciar instância
operator-you-are = Você é quem opera esta instância do Weaveling.
operator-become-heading = Tornar-se o operador
operator-become-hint = Digite o código que o servidor escreveu no log ao iniciar.
operator-code = Código
operator-become-confirm = Tornar-me o operador
operator-no-accounts = Esta instância do Weaveling não tem contas, então não tem operador.
refused-account-locked = Esta conta está bloqueada.
refused-not-an-operator = Só quem opera esta instância do Weaveling pode fazer isso.
refused-operator-cannot-be-locked = Quem opera a instância não pode ser bloqueado.
refused-lock-reason-blank = Bloquear uma conta exige um motivo.
refused-lock-reason-control-character = Um motivo não pode conter caracteres de controle.
refused-lock-reason-too-long = Um motivo pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-account-id-invalid = Esse não é um identificador de conta válido.
refused-account-not-found = Essa conta não existe.
instance-sections = Seções
instance-accounts-heading = Contas
instance-accounts-more = Mostrar mais
instance-account-signed-up = Cadastrada em { $when }
instance-account-unverified = Endereço não confirmado
instance-invitations-heading = Convites
instance-invite-email = E-mail
instance-invite-confirm = Convidar
instance-invitation-withdraw = Retirar
instance-invitation-invite-again = Convidar de novo
instance-invitations-none = Ninguém está convidado no momento.
instance-invitations-more = Mostrar mais convites
instance-invitation-issued = Convidado em { $when }
instance-invitation-open = Aberto até { $when }
instance-invitation-expired = Expirou em { $when }
instance-invitation-declined-address-has-account = Não é preciso: este e-mail já tem uma conta
instance-invitation-declined-turned-down = Recusado
instance-account-operator = Opera a instância
instance-account-locked = Bloqueada desde { $when }
instance-account-lock = Bloquear…
instance-account-unlock = Desbloquear
instance-account-leaving = Será excluída em { $when }
instance-account-delete = Excluir a conta…
instance-account-keep = Manter esta conta
instance-delete-dialog = Excluir a conta de { $name }?
instance-delete-hint = { $name } sai de todos os lugares e não consegue entrar. A conta é mantida por 30 dias, até { $until }, e até lá você pode mantê-la. Depois disso, ela é excluída junto com os projetos dos quais só essa pessoa é membro. Antes, confirme fora do Weaveling que isso é desejado. O histórico da conta mostra que foi você.
instance-delete-hand-over = { $name } é a última pessoa dona de projetos dos quais outras pessoas são membros. Primeiro, essa pessoa precisa passá-los adiante.
instance-delete-cancel = Cancelar
instance-delete-confirm = Excluir a conta
instance-lock-dialog = Bloquear { $name }
instance-lock-hint = { $name } sai da sessão na hora e não pode entrar de novo até a conta ser desbloqueada. O motivo fica salvo com o bloqueio e aparece aqui.
instance-lock-reason = Motivo
instance-lock-cancel = Cancelar
instance-lock-confirm = Bloquear
refused-last-operator = O último operador não pode abrir mão do papel, senão ninguém poderia gerenciar esta instância do Weaveling.
refused-operator-address-not-verified = Só uma conta com endereço confirmado pode operar a instância.
refused-locked-cannot-become-operator = Uma conta bloqueada não pode operar a instância.
refused-signing-up-by-invitation-only = Nesta instância do Weaveling só é possível criar uma conta por convite.
refused-invitation-invalid = O link deste convite é inválido ou não funciona mais.
refused-invitation-expired = Este convite expirou. Peça um novo.
refused-invitation-paused = O projeto deste convite está na lixeira. O convite poderá ser aceito assim que o projeto for restaurado.
refused-email-address-has-account = Este e-mail já tem uma conta. Entre com ela.
refused-invitation-for-another-address = Este convite foi enviado para outro endereço. Entre com esse endereço para aceitá-lo.
refused-invitation-not-by-an-operator = Nesta instância do Weaveling só é possível criar uma conta com um convite de quem a opera. Peça que convide você.
refused-totp-unavailable = Esta instância do Weaveling ainda não oferece o login em duas etapas. Peça a quem a opera que o configure.
refused-totp-nothing-being-set-up = É preciso começar de novo a configurar o login em duas etapas.
refused-totp-code-wrong = Esse não é o código que seu app mostra. Espere o próximo e tente de novo.
refused-recovery-code-wrong = Esse não é um dos seus códigos de recuperação, ou ele já foi usado.
refused-no-second-factor = Esta conta não tem login em duas etapas.
refused-keys-unavailable = Esta instância do Weaveling ainda não pode adicionar chaves. Peça a quem a opera que defina o endereço público dela.
refused-no-key-being-added = É preciso começar de novo a adicionar a chave.
refused-no-key-being-asked = É preciso começar de novo a pedir a chave.
refused-key-refused = A resposta do seu navegador não criou uma chave para esta instância do Weaveling. Tente de novo.
refused-key-held-already = Esta chave já pertence a uma conta.
refused-no-such-key = Esta conta não tem essa chave.
refused-key-id-invalid = Esse não é um identificador de chave válido.
refused-key-name-taken = Outra das suas chaves já tem este nome.
refused-key-name-blank = Uma chave precisa de um nome.
refused-key-name-control-character = O nome de uma chave não pode conter caracteres de controle.
refused-key-name-too-long = O nome de uma chave pode ter no máximo { $max } caracteres; este tem { $actual }.
refused-too-many-keys = Uma conta tem no máximo 20 chaves. Remova uma primeiro.
refused-password-wrong = Essa não é a sua senha.
refused-projects-to-hand-over = Você é a última pessoa dona de projetos dos quais outras pessoas são membros. Primeiro, torne outra pessoa dona de cada um.
refused-no-deletion-pending = Não há exclusão pendente para esta conta.
refused-deletion-asked-by-the-operator = O operador desta instância do Weaveling pediu a exclusão desta conta, então só o operador pode cancelá-la.
refused-preferred-way-unknown = Escolha o app, uma chave ou a forma que você usou da última vez.
refused-key-role-unknown = Uma chave é ou uma chave de acesso ou uma segunda etapa.
instance-account-appoint = Tornar operador
instance-account-dismiss = Tirar o papel de operador
instance-appoint-dialog = Tornar { $name } operador
instance-appoint-hint = { $name } poderá então gerenciar esta instância do Weaveling como você: listar as contas, bloqueá-las e desbloqueá-las, e nomear e dispensar operadores.
instance-appoint-confirm = Tornar operador
instance-dismiss-dialog = Tirar o papel de operador de { $name }
instance-dismiss-hint = { $name } não poderá mais gerenciar esta instância do Weaveling.
instance-dismiss-yourself-hint = Você não poderá mais gerenciar esta instância do Weaveling, e só outro operador poderá devolver o papel.
instance-dismiss-confirm = Tirar o papel
instance-role-cancel = Cancelar
refused-public-url-missing = Esta instância do Weaveling não sabe onde é servida, então seus convites não levariam a lugar nenhum. Peça a quem a opera para definir seu endereço público.
refused-invitation-id-invalid = Esse não é um identificador de convite válido.
refused-invitation-not-found = Esse convite não existe.
refused-invitation-already-accepted = Este convite já foi aceito.
refused-too-late-to-try-again = Esta mudança é antiga demais para ser tentada de novo.

members-link = Membros
members-link-of = Membros: { $names }
share = Compartilhar
share-title = Compartilhar “{ $project }”
share-people = Quem tem acesso
share-invited = Convites
share-all-members = Todos os membros e convites
members-heading = Membros
members-hint = Quem pode ler e escrever neste projeto.
members-you = (você)
members-unnamed = Nome ainda desconhecido
members-role-owner = Dono(a)
members-role-author = Autor(a)
members-role-reader = Leitor(a)
roles-changed = Seu papel foi alterado para { $role }.
members-roles = Quem é dono(a) convida, muda os papéis e exclui o projeto. Quem é autor(a) lê e escreve. Quem é leitor(a) só lê.
members-role-of = Papel de { $name }
members-remove = Remover
members-leave = Sair
members-sections = Seções
members-people = Pessoas
members-invited = Convites
members-invite-as = Como
members-only-owner = Dono(a), a única pessoa
members-last-owner-heading = Você é a única pessoa dona
members-last-owner-hint = Antes de sair, torne outra pessoa dona. Senão ninguém poderia convidar, mudar os papéis ou mover este projeto para a lixeira.
members-new-owner = Nova pessoa dona
members-candidate = { $name } ({ $role })
members-stay = Ficar
members-hand-over = Passar e sair
members-alone-heading = Não há mais ninguém
members-alone-hint = Este projeto não tem outros membros, então, se você sair, não sobra ninguém nele. Mova-o para a lixeira em vez disso, onde pode ser restaurado por 30 dias, ou convide alguém e passe para essa pessoa primeiro.
members-trash-project = Mover o projeto para a lixeira

decimal-point = { "," }
size-bytes = { $amount }{ "\u00A0" }B
size-kilobytes = { $amount }{ "\u00A0" }KB
size-megabytes = { $amount }{ "\u00A0" }MB
size-gigabytes = { $amount }{ "\u00A0" }GB
size-terabytes = { $amount }{ "\u00A0" }TB

refused-asset-id-invalid = Esse não é um identificador de imagem válido.
refused-asset-not-an-image = Este arquivo não é uma imagem que o Weaveling possa usar. Envie uma imagem JPEG, PNG, GIF ou WebP.
refused-asset-image-unreadable = Não foi possível ler esta imagem. Talvez ela esteja danificada.
refused-asset-image-too-large = Esta imagem tem pixels demais. Ela pode ter no máximo 50 megapixels.
refused-asset-limit-reached =
    { $bound ->
        [upload] Uma imagem pode ter no máximo { $most }.
        [instance] Esta instância do Weaveling não tem mais espaço para esta imagem. Ela ocupa { $used } de { $most }.
        [account] Você não tem mais espaço para esta imagem. Você ocupa { $used } de { $most }.
        [project] Este projeto não tem mais espaço para esta imagem. Ele ocupa { $used } de { $most }.
       *[other] Não há mais espaço para esta imagem.
    }
refused-asset-no-account = Só uma conta conectada pode ter imagens próprias.
refused-asset-limit-uncountable = Esse limite é grande demais.
refused-asset-sort-unknown = A lista não pode ser ordenada assim.
refused-asset-cursor-invalid = A lista não pode continuar a partir daí. Carregue-a de novo.

your-account-storage = Suas mídias e o espaço que elas ocupam:
storage-heading = Armazenamento
storage-used = { $used } de { $most } usados
storage-used-unbounded = { $used } usados
storage-hint = Todas as mídias que você enviou, para projetos e como suas. Exclua o que não precisa mais para liberar espaço.
storage-sort = Ordenar por
storage-sort-newest = Mais recentes primeiro
storage-sort-largest = Maiores primeiro
storage-none = Você ainda não enviou nada.
storage-own = Seu próprio arquivo
storage-in-project = Em { $project }
storage-in-a-project = Em um projeto
storage-detail = { $size } · enviada em { $when }
storage-more = Mostrar mais
storage-delete = Excluir
storage-not-deletable = Só os autores do projeto podem excluí-la.
storage-delete-heading = Excluir este arquivo?
storage-delete-hint = O arquivo é excluído para sempre. Isso não pode ser desfeito.
storage-delete-cancel = Cancelar
storage-delete-confirm = Excluir o arquivo

size-unit-megabytes = MB
size-unit-gigabytes = GB
instance-storage-hint = Quanto espaço as mídias ocupam nesta instância do Weaveling.
instance-storage-accounts = Contas
instance-storage-find = Buscar uma conta
instance-storage-find-hint = Nome ou e-mail
instance-storage-none = Nenhuma conta ocupa espaço ainda.
instance-storage-no-match = Nenhuma conta corresponde.
instance-storage-own-limit = Limite próprio
instance-storage-change-limit = Alterar o limite
instance-storage-limit-heading = Limite de armazenamento de { $name }
instance-storage-limit-hint = Sem um limite próprio, a conta tem o limite padrão para contas.
instance-storage-limit-amount = Limite
instance-storage-limit-unit = Unidade
instance-storage-limit-invalid = Digite um número maior que 0.
instance-storage-limit-cancel = Cancelar
instance-storage-limit-default = Usar o limite padrão
instance-storage-limit-save = Salvar
