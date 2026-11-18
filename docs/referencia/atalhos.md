---
type: Reference
title: "Atalhos de teclado"
description: "Todas as teclas do keymap padrão do app, por contexto, com o título de cada comando."
tags: [referência, teclado, atalhos]
generated:
  by: process:tools/gen-atalhos.py
  at: 2026-11-18T23:47:57Z
fontes: [crates/wadawko-app/assets/keymap-default.json, crates/wadawko-app/src/keys/registry.rs]
---

# Atalhos de teclado

> Página **gerada** por `tools/gen-atalhos.py` a partir do keymap padrão — não edite à mão;
> rode o script depois de mudar teclas ou títulos (`--check` acusa quando está desatualizada).

Para mudar uma tecla, crie `~/.config/wadawko/keymap.json` no mesmo formato do
[keymap padrão](https://github.com/aqilputi/wadawko/blob/main/crates/wadawko-app/assets/keymap-default.json)
(paleta → *Criar keymap do usuário*); ele é somado ao padrão e conflitos aparecem na página de Configurações.
Todo comando também pode ser executado pela paleta (`Ctrl+K`), mesmo sem tecla.
Para os conceitos por trás (modos, seleção, arrasto), veja o [guia de uso](/guias/uso.md).

## Em qualquer lugar

Valem em todas as páginas, com ou sem vim.

| Tecla | Comando | Ação |
|---|---|---|
| `Ctrl+K` | `keys::TogglePalette` | Paleta de comandos |
| `Ctrl+P` | `keys::TogglePalette` | Paleta de comandos |
| `Ctrl+Space` | `hints::Jump` | Saltar para um elemento |
| `Alt+F` | `hints::Jump` | Saltar para um elemento |
| `Ctrl+Z` | `project::Undo` | Desfazer |
| `Ctrl+Shift+Z` | `project::Redo` | Refazer |
| `Ctrl+Y` | `project::Redo` | Refazer |
| `Ctrl+N` | `project::New` | Nova música |
| `Ctrl+E` | `project::ExportJson` | Exportar música (.taiko.json) |
| `Ctrl+O` | `project::ImportJson` | Importar música (.taiko.json) |
| `Ctrl+Shift+E` | `project::ExportMp3` | Exportar MP3 |
| `Ctrl+1` | `view::Grade` | Ver: grade |
| `Ctrl+2` | `view::Partitura` | Ver: partitura |
| `Ctrl+3` | `view::Esteira` | Ver: esteira |
| `Ctrl+Enter` | `transport::PlayFromStart` | Tocar do início |
| `Ctrl+B` | `view::ToggleNav` | Barra lateral: mostrar/esconder |
| `f1` | `keys::ShowShortcuts` | Mapa de atalhos |
| `Ctrl+/` | `keys::ToggleRootHints` | Dica de atalhos (o que posso apertar aqui?) |
| `Ctrl+,` | `nav::Config` | Configurações e mapa de atalhos |

## Editor

Valem na área do editor (grade, partitura, esteira), com ou sem vim.

| Tecla | Comando | Ação |
|---|---|---|
| `←` | `grid::Left` | célula ← |
| `→` | `grid::Right` | célula → |
| `↑` | `grid::Up` | linha ↑ |
| `↓` | `grid::Down` | linha ↓ |
| `Shift+←` | `grid::ExtendLeft` | estender seleção ← |
| `Shift+→` | `grid::ExtendRight` | estender seleção → |
| `Shift+↑` | `grid::ExtendUp` | estender seleção ↑ |
| `Shift+↓` | `grid::ExtendDown` | estender seleção ↓ |
| `Home` | `grid::MeasureStart` | início do compasso |
| `End` | `grid::MeasureEnd` | fim do compasso |
| `Ctrl+Home` | `grid::SongStart` | início da música |
| `Ctrl+End` | `grid::SongEnd` | fim da música |
| `PgUp` | `grid::Up` | linha ↑ |
| `PgDn` | `grid::Down` | linha ↓ |
| `Alt+←` | `grid::MoveSelLeft` | Mover seleção ← |
| `Alt+→` | `grid::MoveSelRight` | Mover seleção → |
| `Alt+↑` | `grid::MoveSelUp` | Mover seleção ↑ |
| `Alt+↓` | `grid::MoveSelDown` | Mover seleção ↓ |
| `Alt+Shift+←` | `grid::CloneSelLeft` | Clonar seleção ← |
| `Alt+Shift+→` | `grid::CloneSelRight` | Clonar seleção → |
| `Alt+Shift+↑` | `grid::CloneSelUp` | Clonar seleção ↑ |
| `Alt+Shift+↓` | `grid::CloneSelDown` | Clonar seleção ↓ |
| `Ctrl+S` | `project::Save` | Gravar agora |
| `Tab` | `grid::NextCell` | grava e vai para a próxima célula |
| `Shift+Tab` | `grid::PrevCell` | grava e volta uma célula |
| `Ctrl+L` | `transport::LoopSelection` | Loop: trecho da seleção (ou do compasso do cursor) |
| `Alt+T` | `transport::TapTempo` | Tap tempo (bata no ritmo para marcar o BPM) |
| `Ctrl+Shift+R` | `fx::Roll` | Efeito: rulo (×2 → 3 → 4 → 6 → 8 → nenhum) |
| `Ctrl+Shift+F` | `fx::Flam` | Efeito: flam (liga/desliga) |
| `Ctrl+Shift+X` | `fx::Cut` | Efeito: corte / abafar (liga/desliga) |
| `Ctrl+Shift+.` | `fx::Later` | Efeito: atrasar 10 % |
| `Ctrl+Shift+,` | `fx::Earlier` | Efeito: antecipar 10 % |
| `Ctrl+Shift+V` | `fx::Softer` | Efeito: mais fraco (força 9 → 0 = ghost) |
| `Ctrl+Shift+0` | `fx::Clear` | Efeitos: limpar |
| `Ctrl+Shift+Y` | `fx::Edit` | Efeitos da célula… (V R F D X H) |
| `Ctrl+Alt+↑` | `view::RowTaller` | Linhas mais altas |
| `f10` | `view::Director` | Modo projeção (tela cheia para a TV do ensaio) |
| `Ctrl+Alt+↓` | `view::RowShorter` | Linhas mais baixas (cabem mais linhas) |
| `Ctrl+Shift+L` | `practice::Save` | Trechos: guardar o trecho de loop com um nome… |

## Editor — modo simples (vim desligado)

O modo padrão para quem não usa vim: digite as sílabas direto nas células.

| Tecla | Comando | Ação |
|---|---|---|
| `Ctrl+C` | `grid::Copy` | Copiar seleção |
| `Ctrl+X` | `grid::Cut` | Recortar seleção |
| `Ctrl+V` | `grid::Paste` | Colar |
| `Delete` | `grid::DeleteSelection` | Apagar seleção |
| `Ctrl+Shift+S` | `grid::ToggleSelectMode` | Modo: editar / selecionar |
| `Ctrl+F` | `vim::Search` | Buscar sílaba (/) |
| `Ctrl+Shift+K` | `grid::ToggleKyaiLane` | Editar kiai / notas (linha do cursor) |
| `+` | `transport::BpmUp` | BPM +1 |
| `-` | `transport::BpmDown` | BPM −1 |

## Vim — normal e visual

Com o vim ligado (Configurações ou `keys::ToggleVim`).

| Tecla | Comando | Ação |
|---|---|---|
| `h` | `grid::Left` | célula ← |
| `l` | `grid::Right` | célula → |
| `j` | `grid::Down` | linha ↓ |
| `k` | `grid::Up` | linha ↑ |
| `w` | `grid::NextBeat` | próximo tempo |
| `b` | `grid::PrevBeat` | tempo anterior |
| `e` | `grid::EndOfBeat` | fim do tempo |
| `Shift+W` | `grid::NextMeasure` | próximo compasso |
| `Shift+B` | `grid::PrevMeasure` | compasso anterior |
| `Shift+E` | `grid::MeasureEnd` | fim do compasso |
| `$` | `grid::MeasureEnd` | fim do compasso |
| `_` | `grid::FirstFilled` | 1ª célula preenchida |
| `(` | `grid::PrevSection` | seção anterior |
| `)` | `grid::NextSection` | próxima seção |
| `g` `g` | `grid::SongStart` | início da música |
| `Shift+G` | `grid::SongEnd` | fim da música |
| `g` `k` | `grid::FirstTrack` | primeira linha |
| `g` `j` | `grid::LastTrack` | última linha |
| `Ctrl+D` | `grid::HalfRight` | meia tela → |
| `Ctrl+U` | `grid::HalfLeft` | meia tela ← |
| `1` | `vim::Digit1` | contagem |
| `2` | `vim::Digit2` | contagem |
| `3` | `vim::Digit3` | contagem |
| `4` | `vim::Digit4` | contagem |
| `5` | `vim::Digit5` | contagem |
| `6` | `vim::Digit6` | contagem |
| `7` | `vim::Digit7` | contagem |
| `8` | `vim::Digit8` | contagem |
| `9` | `vim::Digit9` | contagem |
| `!` | `grid::ToggleSfz` | Alternar sforzando (!) |
| `g` `Shift+U` | `grid::Accent` | Acentuar (MAIÚSCULA) |
| `g` `u` | `grid::Unaccent` | Desacentuar (minúscula) |
| `z` `i` | `view::ZoomIn` | Zoom + |
| `z` `o` | `view::ZoomOut` | Zoom − |
| `z` `0` | `view::ZoomReset` | Zoom 100% |
| `z` `k` | `view::RowTaller` | Linhas mais altas |
| `z` `j` | `view::RowShorter` | Linhas mais baixas (cabem mais linhas) |
| `z` `=` | `view::RowReset` | Altura das linhas: padrão |
| `z` `a` | `section::ToggleCollapse` | Recolher/expandir seção |
| `z` `Shift+R` | `section::ExpandAll` | Expandir todas as seções |
| `z` `Shift+M` | `section::CollapseAll` | Recolher todas as seções |
| `Enter` | `transport::PlayHere` | Tocar do cursor |
| `Esc` | `keys::Cancel` | cancelar |
| `"` | `vim::PushRegister` | registrador (\ |
| `Space` `Space` | `transport::TogglePlay` | Tocar / parar |
| `Space` `/` | `keys::TogglePalette` | Paleta de comandos |
| `Space` `j` | `hints::Jump` | Saltar para um elemento |
| `Space` `?` | `keys::ShowShortcuts` | Mapa de atalhos |
| `Space` `,` | `nav::Config` | Configurações e mapa de atalhos |
| `Space` `t` `p` | `transport::TogglePlay` | Tocar / parar |
| `Space` `t` `s` | `transport::Stop` | Parar |
| `Space` `t` `l` | `transport::ToggleLoop` | Loop: ligar/desligar |
| `Space` `t` `Shift+L` | `transport::LoopSelection` | Loop: trecho da seleção (ou do compasso do cursor) |
| `[` | `transport::LoopStartHere` | Loop: começa no compasso do cursor |
| `]` | `transport::LoopEndHere` | Loop: termina no compasso do cursor |
| `Space` `t` `m` | `transport::ToggleMetronome` | Metrônomo: ligar/desligar |
| `Space` `t` `c` | `transport::ToggleCountIn` | Count-in: ligar/desligar |
| `Space` `t` `r` | `transport::ToggleCountInLoop` | Count-in a cada volta do loop: ligar/desligar |
| `Space` `f` `r` | `fx::Roll` | Efeito: rulo (×2 → 3 → 4 → 6 → 8 → nenhum) |
| `Space` `f` `Shift+R` | `fx::RollCresc` | Efeito: rulo crescendo / decrescendo |
| `Space` `f` `f` | `fx::Flam` | Efeito: flam (liga/desliga) |
| `Space` `f` `d` | `fx::Later` | Efeito: atrasar 10 % |
| `Space` `f` `Shift+D` | `fx::Earlier` | Efeito: antecipar 10 % |
| `Space` `f` `x` | `fx::Cut` | Efeito: corte / abafar (liga/desliga) |
| `Space` `f` `v` | `fx::Softer` | Efeito: mais fraco (força 9 → 0 = ghost) |
| `Space` `f` `h` | `fx::Exact` | Efeito: exato, sem humanização (liga/desliga) |
| `Space` `f` `c` | `fx::Clear` | Efeitos: limpar |
| `Space` `f` `e` | `fx::Edit` | Efeitos da célula… (V R F D X H) |
| `Space` `v` `x` | `fx::ToggleColumn` | Efeitos: mostrar a coluna em todas as linhas |
| `Space` `t` `g` | `practice::Save` | Trechos: guardar o trecho de loop com um nome… |
| `Space` `t` `a` | `transport::MetroPattern` | Metrônomo: padrão de acentos… (ABBB) |
| `Space` `t` `d` | `transport::MetroSubdiv` | Metrônomo: subdivisão (1 → 2 → 3 → 4 por tempo) |
| `Space` `t` `v` | `transport::MetroVolume` | Metrônomo: volume… |
| `Space` `t` `k` | `transport::MetroSound` | Metrônomo: timbre (bipe → shime → kane) |
| `Space` `t` `.` | `transport::TapTempo` | Tap tempo (bata no ritmo para marcar o BPM) |
| `Space` `t` `t` | `transport::ToggleTrainer` | Treino (rampa de tempo): ligar/desligar |
| `Space` `t` `w` | `transport::ToggleSwing` | Reto / swing |
| `Space` `t` `+` | `transport::BpmUp` | BPM +1 |
| `Space` `t` `-` | `transport::BpmDown` | BPM −1 |
| `Space` `s` `n` | `section::Add` | + Seção |
| `Space` `s` `c` | `section::Clone` | Clonar seção |
| `Space` `s` `d` | `section::Remove` | Remover seção |
| `Space` `s` `+` | `section::RepeatUp` | Ritornello + |
| `Space` `s` `-` | `section::RepeatDown` | Ritornello − |
| `Space` `s` `m` | `section::MeasureAdd` | + Compasso (seção do cursor) |
| `Space` `s` `Shift+M` | `section::MeasureRemove` | − Compasso (seção do cursor) |
| `Space` `s` `k` | `section::MoveUp` | Mover seção para cima |
| `Space` `s` `j` | `section::MoveDown` | Mover seção para baixo |
| `Space` `s` `r` | `section::Rename` | Renomear seção… |
| `Space` `s` `p` | `section::ToggleSelect` | Tocar só esta seção (selecionar/desselecionar) |
| `Space` `s` `a` | `transport::ClearRegion` | Tocar a música inteira (limpar trecho de loop e seções selecionadas) |
| `Space` `i` `n` | `track::Add` | + Linha (instrumento escolhido) |
| `Space` `i` `d` | `track::Remove` | Remover linha do cursor |
| `Space` `i` `m` | `track::ToggleMute` | Mute da linha do cursor |
| `Space` `i` `s` | `track::ToggleSolo` | Solo da linha do cursor |
| `Space` `i` `i` | `track::CycleInstrument` | Trocar instrumento de + linha |
| `Space` `i` `r` | `track::Rename` | Renomear linha… |
| `Space` `i` `k` | `track::MoveUp` | Mover linha para cima |
| `Space` `i` `j` | `track::MoveDown` | Mover linha para baixo |
| `Space` `i` `e` | `track::Settings` | Editar linha (nome, instrumento, pessoas…) |
| `Space` `i` `f` | `track::ToggleDynRow` | Dinâmica própria da linha: mostrar/esconder |
| `Space` `i` `y` | `track::ToggleKyaiRow` | Kiai da linha: mostrar/esconder |
| `Space` `i` `Shift+M` | `track::ToggleMain` | Linha principal ★ |
| `Space` `v` `g` | `view::Grade` | Ver: grade |
| `Space` `v` `p` | `view::Partitura` | Ver: partitura |
| `Space` `v` `e` | `view::Esteira` | Ver: esteira |
| `Space` `v` `t` | `view::ToggleTheme` | Tema: claro/escuro |
| `Space` `v` `l` | `view::ToggleLayout` | Editor: vertical / horizontal |
| `Space` `g` `e` | `nav::Editor` | Ir para Editor |
| `Space` `g` `m` | `nav::Mixer` | Ir para Mixer |
| `Space` `g` `p` | `nav::Pessoas` | Ir para Pessoas |
| `Space` `g` `k` | `nav::Kit` | Ir para Kit |
| `Space` `u` `2` | `song::Sub2` | Subdivisão: colcheia |
| `Space` `u` `4` | `song::Sub4` | Subdivisão: semicolcheia |
| `Space` `u` `8` | `song::Sub8` | Subdivisão: fusa |
| `Space` `d` `d` | `measure::CycleDynamic` | Dinâmica do compasso: próxima |
| `Space` `d` `<` | `measure::ToggleCresc` | Crescendo no compasso |
| `Space` `d` `>` | `measure::ToggleDim` | Diminuendo no compasso |
| `Space` `a` | `grid::AssignToPen` | Atribuir seleção à pessoa da caneta |
| `Space` `n` | `grid::NextPen` | Caneta: próxima pessoa |
| `Space` `k` | `grid::ToggleKyaiLane` | Editar kiai / notas (linha do cursor) |
| `Space` `t` `b` | `transport::SetBpm` | Definir BPM… |
| `Space` `m` `r` | `project::Rename` | Renomear música… |
| `Space` `m` `n` | `project::NextSong` | Próxima música |
| `Space` `m` `p` | `project::PrevSong` | Música anterior |
| `Space` `m` `d` | `project::Duplicate` | Duplicar música |
| `Space` `z` `i` | `view::ZoomIn` | Zoom + |
| `Space` `z` `o` | `view::ZoomOut` | Zoom − |
| `Space` `z` `0` | `view::ZoomReset` | Zoom 100% |

## Vim — com contagem digitada

| Tecla | Comando | Ação |
|---|---|---|
| `0` | `vim::Digit0` | contagem |

## Vim — sem contagem

| Tecla | Comando | Ação |
|---|---|---|
| `0` | `grid::MeasureStart` | início do compasso |

## Vim — só no modo normal

| Tecla | Comando | Ação |
|---|---|---|
| `i` | `vim::Insert` | inserir |
| `a` | `vim::Append` | inserir depois |
| `Shift+I` | `vim::InsertMeasureStart` | inserir no início do compasso |
| `Shift+A` | `vim::AppendMeasureEnd` | inserir no fim do compasso |
| `s` | `vim::Substitute` | reescrever célula |
| `v` | `vim::Visual` | visual |
| `Shift+V` | `vim::VisualMeasure` | visual por compasso |
| `x` | `grid::DeleteCell` | apagar célula |
| `Shift+X` | `grid::DeleteCellBack` | apagar célula anterior |
| `d` `d` | `grid::DeleteMeasureRow` | compasso (linha) |
| `d` `w` | `grid::DeleteBeat` | tempo |
| `d` `$` | `grid::DeleteToMeasureEnd` | até o fim do compasso |
| `d` `a` `m` | `grid::DeleteMeasureAll` | compasso (todas as linhas) |
| `d` `i` `s` | `grid::DeleteSectionRow` | seção (linha) |
| `Shift+D` | `grid::DeleteToMeasureEnd` | até o fim do compasso |
| `c` `c` | `vim::ChangeMeasureRow` | compasso (linha) |
| `c` `w` | `vim::ChangeBeat` | tempo |
| `y` `y` | `grid::YankMeasureRow` | compasso (linha) |
| `y` `w` | `grid::YankBeat` | tempo |
| `y` `i` `s` | `grid::YankSectionRow` | seção (linha) |
| `Shift+Y` | `grid::YankMeasureRow` | compasso (linha) |
| `p` | `grid::PasteAfter` | colar depois |
| `Shift+P` | `grid::PasteBefore` | colar antes |
| `u` | `project::Undo` | Desfazer |
| `Ctrl+R` | `project::Redo` | Refazer |
| `.` | `vim::Repeat` | Repetir última mudança (.) |
| `m` | `vim::SetMark` | marcar posição (m + letra) |
| `'` | `vim::GotoMark` | ir para marca (' + letra) |
| `/` | `vim::Search` | Buscar sílaba (/) |
| `?` | `vim::SearchBack` | buscar para trás (?) |
| `n` | `vim::SearchNext` | Próxima ocorrência (n) |
| `Shift+N` | `vim::SearchPrev` | Ocorrência anterior (N) |
| `:` | `vim::CommandLine` | Linha de comando (:) |
| `f` | `hints::JumpTrack` | Saltar para célula da linha |
| `Shift+F` | `hints::JumpCells` | Saltar para qualquer célula |

## Vim — só no visual

| Tecla | Comando | Ação |
|---|---|---|
| `d` | `grid::DeleteSelection` | Apagar seleção |
| `x` | `grid::DeleteSelection` | Apagar seleção |
| `y` | `grid::Copy` | Copiar seleção |
| `c` | `vim::ChangeSelection` | reescrever seleção |
| `p` | `grid::Paste` | Colar |
| `o` | `vim::SwapAnchor` | trocar ponta da seleção |
| `v` | `keys::Cancel` | cancelar |
| `Shift+V` | `vim::VisualMeasure` | visual por compasso |

## Vim — inserção

| Tecla | Comando | Ação |
|---|---|---|
| `Esc` | `vim::Normal` | voltar ao normal |
| `Ctrl+[` | `vim::Normal` | voltar ao normal |

## Paleta de comandos (Ctrl+K)

Com a paleta aberta. Tab/Shift+Tab e ↑/↓ andam na lista, Enter executa, Esc fecha.

| Tecla | Comando | Ação |
|---|---|---|
| `Ctrl+T` | `view::ToggleTheme` | Tema: claro/escuro |
| `Ctrl+V` | `keys::ToggleVim` | Vim: ligar/desligar |
| `Ctrl+L` | `transport::ToggleLoop` | Loop: ligar/desligar |
| `Ctrl+M` | `transport::ToggleMetronome` | Metrônomo: ligar/desligar |
| `Ctrl+S` | `keys::ShowShortcuts` | Mapa de atalhos |
| `Tab` | `keys::PaletteNext` | próximo comando |
| `Shift+Tab` | `keys::PalettePrev` | comando anterior |

## Contexto `Workspace && layout == vertical`

| Tecla | Comando | Ação |
|---|---|---|
| `↓` | `grid::Right` | célula → |
| `↑` | `grid::Left` | célula ← |
| `→` | `grid::Down` | linha ↓ |
| `←` | `grid::Up` | linha ↑ |
| `Shift+↓` | `grid::ExtendRight` | estender seleção → |
| `Shift+↑` | `grid::ExtendLeft` | estender seleção ← |
| `Shift+→` | `grid::ExtendDown` | estender seleção ↓ |
| `Shift+←` | `grid::ExtendUp` | estender seleção ↑ |
| `Alt+↓` | `grid::MoveSelRight` | Mover seleção → |
| `Alt+↑` | `grid::MoveSelLeft` | Mover seleção ← |
| `Alt+→` | `grid::MoveSelDown` | Mover seleção ↓ |
| `Alt+←` | `grid::MoveSelUp` | Mover seleção ↑ |
| `Alt+Shift+↓` | `grid::CloneSelRight` | Clonar seleção → |
| `Alt+Shift+↑` | `grid::CloneSelLeft` | Clonar seleção ← |
| `Alt+Shift+→` | `grid::CloneSelDown` | Clonar seleção ↓ |
| `Alt+Shift+←` | `grid::CloneSelUp` | Clonar seleção ↑ |

## Contexto `Workspace && layout == vertical && (mode == normal || mode == visual || mode == vline)`

| Tecla | Comando | Ação |
|---|---|---|
| `j` | `grid::Right` | célula → |
| `k` | `grid::Left` | célula ← |
| `l` | `grid::Down` | linha ↓ |
| `h` | `grid::Up` | linha ↑ |

## Prefixos (which-key)

Apertar o prefixo e esperar mostra as continuações possíveis.

| Prefixo | Grupo |
|---|---|
| `Space` | líder |
| `Space` `t` | transporte |
| `Space` `s` | seção |
| `Space` `i` | instrumento |
| `Space` `v` | ver |
| `Space` `g` | ir para |
| `Space` `u` | subdivisão |
| `Space` `d` | dinâmica |
| `Space` `m` | música |
| `Space` `z` | zoom |
| `Space` `f` | efeitos |
| `g` | ir |
| `z` | zoom/dobrar |
| `d` | apagar |
| `d` `a` | apagar em todas as linhas |
| `d` `i` | apagar na linha |
| `y` | copiar |
| `y` `i` | copiar na linha |
| `c` | reescrever |
