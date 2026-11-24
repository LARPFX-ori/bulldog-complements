# AGENTS.md — Regras de Engenharia e Governança do Bulldog Complements

Estas regras se aplicam a todo contribuidor, humano ou agente de IA, em qualquer repositório do projeto **Bulldog Complements** (coleção de plug-ins de efeitos visuais e transições de vídeo). Elas vêm em três camadas: as Regras da Casa, as regras de Repositórios Rust e as regras específicas dos Plug-ins de Vídeo (§0 a §3). Uma camada posterior pode ser mais rígida que a anterior, nunca mais branda. `CLAUDE.md` contém as instruções de trabalho; este arquivo contém as regras que nunca devem ser quebradas. Quando os dois divergirem, este arquivo prevalece.

---

## Regras da Casa

Estas regras valem em todo repositório do **Bulldog Complements**. Uma regra posterior pode ser mais rígida, não mais branda.

1. **Uma pessoa é responsável.** Quem executa um agente responde por tudo o que ele abre (issues, pull requests, commits) e pelos seus comentários de revisão.
2. **Verifique, depois reivindique.** Antes de escrever código, leia os comentários da issue, pesquise pull requests por `#N` e palavras-chave e confira os commits recentes em `main`. Comente na issue primeiro. Libere uma reivindicação que você não vai concluir.
3. **Confirme, não adivinhe.** Abra a definição de uma função, struct, shader ou parâmetro antes de usá-la. Confirme a causa no código, mesmo quando alguém indicar um local.
4. **Mantenha as correções restritas e testadas.** Corrija apenas o que a issue descreve, onde o bug reside, e adicione um teste que falhe sem a correção.
5. **Revise o diff antes de abrir um pull request.** Os identificadores existem como são usados, o teste falha sem a correção, nada não relacionado foi alterado e a descrição é verdadeira.
6. **Procure um pull request melhor.** Se outro PR corrige a mesma coisa, feche o seu quando o outro for melhor ou equivalente, e diga por quê.
7. **Relate apenas o que foi executado.** Diga o que você testou e o que não conseguiu executar (ex.: build cross-platform, validação em host OFX específico). Sem build ou teste local, diga isso e conte com a CI antes de marcar como pronto.
8. **`main` muda por meio de pull request.** Não faça push em `main`. A proteção de branch impõe isso. Não há exceção.
9. **Pare e pergunte apenas sobre:** licenciamento fora da lista de permissões, publicação (site, pacotes, instaladores), releases, um novo remote, segredos, gastos de dinheiro, `git` destrutivo e qualquer outra coisa difícil de desfazer. Caso contrário, continue trabalhando. `plan/` é um caderno ignorado pelo git, não regras. Sem ele, não peça por ele e não pare.
10. **Mantenha segredos e dados pessoais fora** do repositório, dos logs (`logs/bulldog-complements.log`, `logs/crash.log`) e das requisições.
11. **A CI permanece compartilhada.** Os workflows em `.github/workflows/` devem ser reutilizados. Não altere a CI em um pull request de funcionalidade ou de correção sem necessidade justificada.
12. **Proteções dizem por quê.** Uma verificação que parece removível (lint, guarda de invariante, checagem de CI) diz o que ela previne. Leia isso antes de alterá-la.
13. **As instruções devem ser legíveis por todos.** Nada de notas sobre uma única máquina (caminhos absolutos, configurações locais) e nada de links para arquivos que o leitor não consegue abrir. Não crie links para ele.
14. **Mantenha os pull requests andando.** Pull requests simultâneos são aceitáveis; feedback acionável sem resposta não é. Responda em 7 dias; lembrete aos 14; elegível para fechamento aos 30, contados a partir do momento em que uma solicitação acionável aguarda o autor. Sugestões e achados de bots não confirmados não iniciam a contagem. Uma correção, explicação ou plano acordado a reinicia; atividade não relacionada, não. Aguardar um revisor ou mantenedor a pausa e não conta contra o autor. Um mantenedor decide o fechamento; ele nunca é automático. Reutilize resultados de busca, não faça polling e pare ao atingir um limite de taxa.
15. **Sem atribuição a agentes.** Um pull request ou mensagem de commit não deve declarar que foi escrito por um agente.

---

## Regras Rust e de Plug-ins Nativos

1. **Nunca travar o Host (Zero Panic em Produção).** Plug-ins rodam dentro do processo de editores de vídeo (NLEs). Código fora de `#[cfg(test)]` não entra em pânico e não usa `unsafe`, exceto nas fronteiras FFI de APIs de plug-in (OpenFX / C ABI) e GPU que este arquivo nomeia (§0, regra 3). Retorne `Result<T, E>` (ou códigos de status OFX adequados na fronteira), trate toda entrada como hostil e limite recursão e alocações. Novas raízes de crate de produção declaram os atributos `deny`: `clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`, `clippy::unimplemented`, `clippy::todo`, `clippy::unreachable`. Toda correção de travamento vem com um teste que entrava em pânico antes da correção.
2. **Sala limpa (Clean Room) e Identidade Original.** Aprenda a partir da teoria de processamento de imagens, matemática de shaders, fotografia, óptica e comportamento visual observado em efeitos clássicos de mercado. Nunca copie código, shaders, LUTs, presets, ícones, strings, nomes registrados ou artes de suítes comerciais (Boris FX Continuum, Sapphire, Red Giant Universe/Magic Bullet, NewBlueFX, etc.). Verifique a licença de uma dependência antes de adicioná-la.
3. **Assets, LUTs, Presets e Fontes.** Todo asset embutido (texturas de grão, mapas de lens flare, LUTs, presets de fábrica, ícones) é original ou devidamente licenciado sob a lista SPDX permitida, acompanhado de arquivo `.attribution` e registrado em `ATTRIBUTION.md`. Não faça commit de arquivos de fonte proprietários do sistema operacional.
4. **Núcleo em Rust Nativo e Shaders Portáveis.** O processamento DSP/imagem e a camada de host/OFX são construídos em Rust nativo com shaders portáveis (`wgpu` / WGSL ou fallback em CPU). Nenhuma dependência de runtimes web ou Electron/webview.
5. **Determinismo Temporal e Espacial.** Efeitos dependentes de tempo (ruído animado, partículas, transições, dissolves) devem ser determinísticos dado o índice de quadro (`frame` / `time`), o progresso normalizado (`0.0..=1.0`) e a semente (`seed`). O mesmo quadro renderizado fora de ordem (*random access* na timeline) deve produzir exatamente o mesmo resultado visual.
6. **Créditos de contribuidores.** Adicione ou edite apenas a entrada da sua própria pessoa em `contributors/people.toml`, e somente depois de perguntar a ela. Nunca adivinhe um nome nem edite `contributors/contributors.json` à mão.

---

## Lista de Licenças Permitidas (SPDX)

Dependências (`Cargo.toml`) e assets devem usar identificadores SPDX desta lista (o projeto **Bulldog Complements** é distribuído sob `MIT OR Apache-2.0`):

- `MIT`
- `Apache-2.0`
- `BSD-2-Clause`
- `BSD-3-Clause`
- `ISC`
- `OFL-1.1`
- `Ubuntu-font-1.0`
- `Bitstream-Vera`
- `CC0-1.0`
- `CC-BY-4.0`
- `CC-BY-SA-4.0`
- `LicenseRef-Public-Domain`

---

## 0. Nunca Travar o Host (Precedência Máxima)

Um plug-in que entra em pânico (`panic!`) derruba o editor de vídeo inteiro (Neoveg, VEGAS Pro, DaVinci Resolve, Natron, etc.) e faz o usuário perder trabalho não salvo. Um buffer de quadro inválido, resolução zero, parâmetros `NaN`/infinitos, falha de alocação de GPU ou preset corrompido devem gerar um erro recuperável ou *bypass* limpo da imagem original, **nunca um `panic` e nunca corrupção de memória**. **Esta regra tem precedência sobre qualquer entrega de funcionalidade:** não entregue um efeito adicionando um caminho de pânico, e corrija um travamento antes de construir em cima dele.

1. **Falhe com `Result<T, E>` e Bypass Seguro.** Propague erros internamente com `?` adicionando contexto relevante. Na fronteira do plug-in, registre o erro no log e faça fallback seguro (ex.: copiar o buffer de entrada para saída ou retornar status de erro OFX válido) sem derrubar o processo hospedeiro.
2. **Proibição de macros de pânico em produção:** É proibido usar `unwrap()`, `expect()`, `panic!()`, `unreachable!()`, `todo!()` ou `unimplemented!()` em código não-teste. Use `?`, `ok_or(...)?`, `let Some(x) = ... else { return Err(...) }` ou fallbacks seguros. Efeitos ou parâmetros inacabados retornam erro explícito ou operam em modo *pass-through*. A única exceção é uma chamada comprovadamente infalível apenas pelo código local (ex.: um literal que sempre é analisado com sucesso): um único `#[allow(clippy::expect_used)]` no item, com o motivo.
3. **Política estrita de `unsafe`:** O workspace adota `unsafe_code = "forbid"` por padrão. As únicas exceções permitidas são módulos isolados de fronteira FFI (`#[allow(unsafe_code)]`) devidamente encapsulados em APIs seguras que retornam `Result`:
   - Camada de exportação C ABI / OpenFX (`OfxPlugin`, ponteiros de imagem/clipes fornecidos pelo host);
   - Interoperabilidade de baixo nível com APIs gráficas/GPU quando estritamente necessário.
   - Todo bloco `unsafe` exige comentário `// SAFETY:` justificando ponteiros não nulos, alinhamento, tempo de vida dos buffers de quadro e limites de `row_bytes` (*stride*).
4. **Toda entrada do host e do usuário é hostil.** Buffers de quadro, dimensões (`width`, `height`), `row_bytes` (incluindo *stride* negativo ou maior que `width * pixel_size`), formatos de pixel (RGBA8, RGBA16, RGBA32F, YUVA), parâmetros de UI, curvas/keyframes, textos de geradores e presets externos não são confiáveis:
   - Respeite sempre o `row_bytes` (*stride*) de origem e destino; nunca assuma buffers contíguos (`width * channels`);
   - Indexe e fatie slices com `.get(...)` e strings apenas em limites válidos de caractere;
   - Use aritmética `checked_*` e `saturating_*` para offsets, coordenadas de pixel e tamanhos de buffer;
   - Nunca divida por valores potencialmente nulos (largura, altura, raio de blur, escala, duração da transição);
   - Nunca converta `NaN`, infinito ou negativos para inteiros sem validação (`clamp` seguro);
   - Não chame `clamp` com limites que podem se cruzar (`min > max`);
   - Limite alocações de memória temporária dimensionadas por dimensões de quadro ou parâmetros de raio/partículas.
5. **Limite recursão e laços.** Árvores de nós internos, cadeias de efeitos compostos e sistemas de partículas/raios podem ter parâmetros extremos: imponha limites explícitos de iterações, raio máximo efetivo e contagem máxima de partículas.
6. **Isolamento de threads e locks.** Envenenamento de `Mutex`/`RwLock` não é fatal: use `.lock().unwrap_or_else(PoisonError::into_inner)`. Se um worker ou shader falhar, capture o erro e retorne estado degradado/bypass sem abortar o processo.
7. **Rede de segurança na fronteira FFI.** Nenhum unwind de pânico em Rust pode atravessar a fronteira C ABI (`extern "C"`). Toda função exposta ao host deve envolver sua execução em `std::panic::catch_unwind`, convertendo eventuais falhas inesperadas em códigos de erro OFX (`kOfxStatFailed`).
8. **Prove com testes.** Toda correção de travamento exige um teste de regressão sintético que falhava antes da correção. Parsers de presets e pontos de entrada de renderização recebem testes com entradas hostis (resolução `0x0`, `1x1`, `8192x8192`, `NaN`, valores extremos de parâmetros e strides não contíguos).
9. **Lints obrigatórios:** Crates de produção declaram `#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]`. `clippy.toml` os permite em testes.

---

## 1. Assets e Propriedade Intelectual: Zero Cópia Comercial

Esta regra é absoluta. O projeto implementa suítes de efeitos e transições originais inspiradas nas categorias clássicas da indústria, mas com identidade, matemática, nomes e assets 100% próprios.

1. **Zero assets ou nomes proprietários de terceiros:**
   - Nunca copie, extraia, faça engenharia reversa ou redistribua shaders, LUTs, mapas de deslocamento, texturas de lens flare, presets, ícones ou strings de suítes proprietárias (Boris FX, Sapphire, Continuum, Red Giant, Magic Bullet, NewBlueFX, etc.).
   - **Nomenclatura Original:** Nunca utilize marcas registradas ou nomes comerciais de pacotes de terceiros nos identificadores públicos (`OfxPlugin.pluginIdentifier`), nomes de exibição ou categorias. Utilize o prefixo organizacional `br.com.bulldog.complements.*` (ou `com.bulldog.complements.*`) e nomes originais para cada suíte e efeito.
   - Se a licença de um asset ou algoritmo for desconhecida ou incompatível, ele não entra.
2. **Atribuição obrigatória (Sidecar + Índice):** Para todo arquivo de asset `X` adicionado ao repositório (mapas de textura, LUTs base, fontes para geradores de texto, ícones), crie `X.attribution` ao lado e registre-o em `ATTRIBUTION.md`:
   ```text
   asset: <nome do arquivo>
   title: <descrição>
   author: <criador / detentor dos direitos>
   source: <URL, ou "original work" para assets criados por contribuidores>
   license: <identificador SPDX ou "MIT OR Apache-2.0">
   license-file: <caminho para o texto da licença, se exigido>
   added: <YYYY-MM-DD> por <contribuidor>
   notes: <como foi criado ou adaptado>
   ```
3. **Capturas de tela e mídia de teste:** Screenshots no repositório devem exibir exclusivamente os plug-ins do **Bulldog Complements** rodando em hosts abertos (como Neoveg ou Natron) sobre mídia gerada proceduralmente ou licenciada abertamente (ex.: Blender Foundation, `CC-BY-4.0`, com atribuição). Nunca faça commit de mídia de teste pesada no repositório; gere fixtures sintéticas via código ou `ffmpeg` em `fixtures/` (ignorado pelo git).

---

## 2. Sala Limpa (Clean Room) e Padrão OpenFX

1. **Engenharia reversa proibida:** Nunca descompile, inspecione binários `.ofx` / `.dll` / `.so` ou extraia recursos de pacotes comerciais de plug-ins. Toda implementação deve partir de especificações públicas (padrão OpenFX), literatura de computação gráfica/processamento de sinais e observação visual de comportamento artístico.
2. **Conformidade com o Padrão OpenFX (OFX):**
   - Implemente a ABI OpenFX de acordo com a especificação oficial aberta da Academy Software Foundation / OpenFX.
   - Suporte corretamente o ciclo de vida completo do plug-in: `OfxActionLoad`, `OfxActionDescribe`, `OfxActionDescribeInContext`, `OfxActionCreateInstance`, `OfxActionDestroyInstance`, `OfxImageEffectActionGetRegionOfDefinition`, `OfxImageEffectActionGetRegionsOfInterest`, `OfxImageEffectActionIsIdentity`, `OfxImageEffectActionRender` e `OfxActionUnload`.
   - Implemente `IsIdentity` sempre que os parâmetros resultarem em efeito nulo (ex.: intensidade `0.0`, transição em `0.0` ou `1.0`), permitindo que o host otimize o grafo de renderização sem processar pixels desnecessariamente.
3. **Sem código Copyleft (GPL/LGPL/AGPL):**
   - Nunca copie código de plug-ins ou bibliotecas GPL/LGPL/AGPL (ex.: Frei0r GPL, G'MIC, FFmpeg, OpenCV com módulos não-permissivos).
   - `ffmpeg`/`ffprobe` podem ser usados apenas como ferramentas externas de geração de fixtures de teste e verificação visual fora do binário distribuído.

---

## 3. Arquitetura da Coleção e Módulos de Plug-ins

### 3.1 Estrutura de Suítes (Domínios do Projeto)
A coleção **Bulldog Complements** é organizada em módulos/suítes funcionais independentes que compartilham um núcleo comum de renderização e ABI (`crates/`), empacotados como um `.ofx.bundle` por suíte com prefixo `br.com.bulldog.complements.*`:

1. **Núcleo Compartilhado (`bulldog_core` / `bulldog_ofx` / `bulldog_packager`):**
   - Abstração segura sobre a ABI OpenFX (`OfxPlugin`), gerenciamento de parâmetros, curvas/interpolação, conversões de espaço de cor (Linear, sRGB, Rec.709, Rec.2020), pipeline de imagem (8-bit, 16-bit, 32-bit float), backend híbrido GPU/CPU e empacotamento/verificação `.ofx.bundle`.
2. **Suíte *Continuus* (`continuus`):**
   - Coleção generalista de efeitos de acabamento, estilização, desfoque, distorção, luzes, restauração de imagem, *keying* e compositing multiuso.
3. **Suíte *Sapphirus* (`sapphirus`):**
   - Efeitos ópticos de alta precisão, brilhos (*glows*), *lens flares* procedurais, aberração cromática, distorções orgânicas, iluminação volumétrica e texturas cinematográficas.
4. **Suíte *Dulcia Nitentia* (`dulcia_nitentia`):**
   - Color grading, emulação de película cinematográfica (*film look*), curvas tonais, halation, grão de filme físico, vinhetas ópticas e correção de cor criativa.
5. **Suíte *Caeruleus Nebulosus* (`caeruleus_nebulosus`):**
   - Utilitários de produção, geradores de texto e títulos, estabilização, *picture-in-picture*, ferramentas de ajuste rápido, máscaras/mattes e elementos gráficos essenciais.
6. **Suíte *3D Transitions* (`transitions_3d`):**
   - Transições espaciais e geométricas em 3D (cubos, dobras, páginas, planos rotativos, perspectiva com iluminação e sombras) operando no contexto `OfxImageEffectContextTransition` com parâmetro normalizado `Transition` (`0.0..=1.0`).
7. **Suíte *TrackerMan* (`tracker_man`):**
   - Rastreamento planar, transformações projetivas (*corner pin*), estabilização de superfície, rotoscopia assistida e remoção/inserção de elementos em cena.
8. **Suíte *Glass Brokers* (`glass_brokers`):**
   - Distorções, fraturas, extrusões, facetas geométricas 3D e transições espaciais estilizadas com refração e iluminação direcional.
9. **Suíte *Universum* (`universum`):**
   - Estilização retrô e analógica (VHS, CRT, varredura, *glitch*, desvio de sinal), elementos de HUD procedurais, *motion graphics* e transições estilizadas.
10. **Suíte *Particulae Legales* (`particulae_legales`):**
    - Sistemas determinísticos de partículas procedurais (emissores, brasas, fumaça, faíscas, névoa e atmosferas) calculados de forma reproduzível a partir de quadro/tempo e semente.

### 3.2 Invariantes de Processamento de Imagem e Cor
- **Independência de Resolução (*Resolution Independence*):** Todo parâmetro espacial (raios de blur, posições de centro, tamanhos de partículas, largura de bordas) deve ser normalizado em relação à largura/altura do quadro ou expresso em coordenadas normalizadas (`0.0..=1.0`), nunca em pixels fixos que mudam de aparência entre 720p, 1080p e 4K.
- **Processamento em Ponto Flutuante Linear:** Operações de mistura óptica (*glow*, *flare*, *blur*, *motion blur*, *dissolve*) devem operar internamente em espaço linear `f32` para evitar halos escuros e perda de alcance dinâmico (HDR), convertendo de/para o formato do buffer do host na entrada e saída.
- **Preservação de Canal Alfa (Premultiplied vs Straight):** Respeite o estado de pré-multiplicação informado pelo host (`kOfxImagePropPreMultiplication`). Nunca vaze RGB em pixels totalmente transparentes nem cause bordas escuras em composições.
- **Regiões de Interesse e Definição (RoD / RoI):** Efeitos que expandem os limites da imagem (ex.: *glow*, *drop shadow*, *blur*) devem declarar corretamente `GetRegionOfDefinition` e `GetRegionsOfInterest` para evitar cortes abruptos nas bordas do quadro.

### 3.3 Portões de Qualidade (Quality Gates)
Antes de abrir ou atualizar qualquer pull request, verifique localmente:
1. `cargo fmt --all -- --check`
2. `cargo clippy --workspace --all-targets -- -D warnings`
3. `cargo test --workspace`
4. Testes de validação de metadados e descritores OFX (IDs únicos, faixas de parâmetros válidas `min <= default <= max`, rótulos não vazios).
5. Verificação de licenças e entradas em `ATTRIBUTION.md`.
6. Empacotamento do bundle `.ofx.bundle` (estrutura padrão `Contents/<Arch>/<Plugin>.ofx`) verificando ausência de símbolos indefinidos e isolamento de dependências.

### 3.4 Fluxo de Branches e Commits
- Crie uma branch descritiva a partir de `main` (`feat/<suite>-<efeito>`, `fix/<suite>-<bug>`, `docs/...`).
- **Nunca apague uma branch, local ou remota**, mesmo depois do merge ou do fechamento do PR. Isso inclui `git branch -d`/`-D`, `git push --delete`, `git push origin :<branch>`, `git fetch --prune`/`git remote prune` e a opção "Delete branch" do GitHub. Não reescreva o histórico de uma branch publicada (`git push --force`, `git reset` seguido de push).
- Faça commits atômicos e focados em uma única tarefa por vez, mantendo a árvore sempre compilável e verde.
- **Todo trabalho concluído vai para `main`.** Ao terminar uma tarefa, abra o pull request e faça o merge em `main` assim que a CI estiver verde e as aprovações existirem, sempre via PR (Regras da Casa, regra 8).
- Documente cada novo plug-in ou efeito implementado na matriz de catálogo do projeto (`README.md` e `CATALOG.md`).

### 3.5 Desempenho e Hardware Limitado
Os plug-ins do **Bulldog Complements** devem rodar de forma fluida tanto em estações com GPU dedicada quanto em máquinas modestas com CPU integrada.
- **Fallback obrigatório em CPU:** Todo efeito acelerado por GPU (`wgpu`/shaders) deve possuir uma implementação de referência em CPU testada e equivalente. Se a inicialização da GPU falhar ou não houver adaptador compatível, o plug-in assume o caminho em CPU automaticamente sem falhar a renderização.
- **Zero alocação por pixel e reuso de buffers:** Nunca aloque memória no heap dentro do loop interno de pixels/linhas. Buffers intermediários de passes múltiplos (ex.: pirâmide de *blur*, *mipmaps* de *glow*) devem ser reutilizados por instância de renderização ou gerenciados por pool seguro.
- **Paralelismo seguro:** Utilize paralelismo por linhas/tiles em CPU (ex.: `rayon`) respeitando limites de threads e sem compartilhar estado mutável sem sincronização.
- **Cancelamento cooperativo (`OfxAbort`):** Em efeitos pesados (múltiplos passes, partículas densas), consulte `abort()` do host periodicamente entre passes/blocos de linhas e interrompa o processamento imediatamente se o usuário cancelar ou mover o playhead.
- **SIMD portátil:** Otimizações SIMD devem ser detectadas em tempo de execução, nunca compiladas com flags fixas da máquina de build (`target-cpu=native`).
