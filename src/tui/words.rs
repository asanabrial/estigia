//! What the screen says, in the language somebody chose.
//!
//! # Why the English is the key
//!
//! Every translatable string is written in English **where it is used**, and the
//! translation is looked up by that text. The alternative — an identifier per
//! message, and a table of `MENU_SETUP_ABOUT = "choose agents…"` somewhere else
//! — moves the words a reader needs out of the code that shows them, and this
//! screen's prose is not decoration: half the design arguments in [`super`] are
//! made in a sentence that is on the screen. A reader who has to follow an
//! identifier to a table to find out what a panel says will stop following it.
//!
//! # Why a missing translation is a test failure, not a fallback
//!
//! [`Tongue::say`] falls back to the English, because a screen that panics
//! mid-draw is worse than a screen with one English line on it. That fallback is
//! made unreachable by `every_line_the_screen_says_has_been_translated`, which
//! fails on the first line this table does not carry. It finds them two ways,
//! because the screen says them two ways: the literals written inline are read
//! out of this crate's own source, and the ones that arrive through a method —
//! a setting's explanation, an adapter's caveat — are walked
//! from the lists that declare them. The fallback is the seatbelt; the guard is
//! the reason it is never worn.
//!
//! # What is **not** translated
//!
//! A setting's display label is prose and is translated. Its persisted name is
//! the key of a row in a markdown table an operator edits by hand and names in
//! `estigia config set`; the values that row accepts are canonical too. The
//! screen may translate what somebody sees while the file cells and printed CLI
//! commands retain the exact strings the parser accepts.
//!
//! # Placeholders
//!
//! A line with something interpolated into it is stored with **named** holes —
//! `"{count} of {known} agents chosen"` — and filled at runtime by `fill`.
//! Named rather than positional because a translation reorders: Spanish puts
//! the count where English puts the noun, and a `{0}`/`{1}` scheme makes that a
//! silent swap. `every_translation_carries_the_same_holes_as_its_english`
//! crosses the two sets.

use std::path::{Path, PathBuf};

/// A language the screen speaks.
///
/// Deliberately a closed set, unlike [`crate::config::Language`] — which is free
/// text because the languages an *issue* may be written in are not ours to
/// enumerate. A screen can only speak the languages somebody has written the
/// words for, and offering one this table does not carry would be offering a
/// language that silently renders in English.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Tongue {
    /// English.
    #[default]
    English,
    /// Spanish.
    Spanish,
}

/// Every language the screen speaks, in the order they are offered.
pub const TONGUES: &[Tongue] = &[Tongue::English, Tongue::Spanish];

const HELP_ES: &str = r#"Estigia sostiene las herramientas. Pone puerta a toda escritura en el
repositorio, y a todo límite irreversible, contra una reclamación adjudicada
en el tracker de issues.

  Agentes       todo agente del que Estigia puede sostener las herramientas,
                con su estado. La primera fila, todos los agentes elegidos,
                responde por todos los agentes elegidos a la vez. ⏎ abre el
                agente bajo el cursor: sus ajustes propios, y los modelos
                bajo el Reparto de modelos. Espacio marca o desmarca el agente bajo
                el cursor.
  Opciones      qué es cierto de este repositorio lo pregunte el agente que
                lo pregunte: dónde viven los issues, qué historia debe
                acabar teniendo la rama base, y cuánto puede una escritura
                rutinaria apoyarse en la última comprobación. Debajo, en su
                propio panel, están las preferencias de esta pantalla: cómo
                se ve en esta máquina, sin escribirlas en ningún contrato.
  Guardia       el hook de pre-push. El único límite que ningún agente puede
                rodear, porque corre en git y no en el agente.
  Diagnóstico   lo que una ejecución necesita antes de jurar, para el
                repositorio que muestra esta pantalla: las mismas comprobaciones,
                con las mismas palabras, que `estigia doctor`, y bajo cada
                una que falla, la salida.

Teclas
  ↑↓ o jk     mueve el cursor
  ⏎           abre el agente, o la lista de respuestas de una fila
  ⌫ / Esc     atrás un nivel; Esc sale solo desde el menú
  ←→ o hl     cambia la fila resaltada, sobre la fila misma
  espacio     marca un agente o muestra todas las respuestas de una fila
  r           restaura la fila resaltada a lo instalado
  s           revisa lo que se va a escribir, desde cualquier sitio; ⏎
              confirma y Esc cancela sin escribir nada
  u           desinstala el agente instalado bajo el cursor, o el guard de
              push desde el menú; muestra qué se quitaría y espera ⏎
  ?           esta página, desde cualquier sitio; cualquier tecla vuelve
  q           salir
  Ctrl-C      salir, avisando una vez antes si hay algo sin guardar

Marcas
  *           esta sesión lo movió y aún no lo ha escrito
  ·           este repositorio ya lo fija distinto de lo que trae el skill
  •           junto a Agentes u Opciones en el menú: lleva una de esas marcas
  (not held)  escrito en el contrato, sin ninguna puerta detrás aquí

Casi todos los ajustes tienen dos o tres respuestas y ninguna más, así que
←→ suele ser suficiente: ningún campo y nada que escribir bien. ⏎ o espacio
abre la lista completa; una ruta o un tablero ofrece un campo para lo que
ninguna lista puede contener. Los destinos de modelo son un bloque aparte
bajo los ajustes, con orchestrate, apply y los agentes delegados. Cada fila
abre directamente el catálogo orientativo del
anfitrión; ⏎ o espacio eligen, siempre se puede escribir un ID personalizado
y heredar elimina solo ese destino. La misma lista ofrece también el esfuerzo
con el que corre, y una vez puesto, una entrada para devolvérselo al
anfitrión. Las respuestas compartidas no toman prestado el catálogo de ningún
agente.
La CLI conserva la edición de la celda completa: los modelos en la primera
lista, los esfuerzos en la segunda tras un punto y coma.

Solo Claude Code recibe actualmente definiciones enrutables por el anfitrión de
revisores; Claude Code y OpenCode reciben ambos los roles que
nombre `Model routing` — nombrar un rol es lo que instala su definición, y el
modelo de al lado solo dice con qué corre. Todos los demás anfitriones conservan
estos valores como declaraciones de reparto; orchestrate, apply, judge y una
ruta visible tampoco demuestran ejecución.

Algunas filas valen menos para algunos agentes. Estigia puede poner puerta a
las llamadas de herramienta de un agente cuyo anfitrión se lo permite; el
resto recibe el contrato y la guardia de pre-push, sin nada entre ellos y una
edición. La página propia de cada agente marca esas filas sobre la propia fila.

La lista de agentes muestra la misma pregunta: `gated` es un agente que
Estigia sostiene; `contract only`, uno al que solo puede pedírselo.

La etiqueta visible se traduce. Los nombres persistidos de los ajustes, los
valores aceptados y los comandos CLI impresos permanecen canónicos: son las
claves y celdas de las tablas markdown y los argumentos de `estigia config
set`.

No se escribe nada en un agente hasta que `s` ha mostrado lo que va a escribir
y lo confirmas. Tres teclas actúan al momento: la guardia de push escribe un
hook en el repositorio mostrado, un cambio de idioma se recuerda, y `n` en el
selector de carpetas crea la carpeta en disco. Salir sin `s` no cambia nada
más."#;

impl Tongue {
    /// What this language calls itself.
    ///
    /// Its own name rather than its English one: somebody looking for Spanish
    /// on a screen they cannot read is looking for `Español`.
    pub fn name(self) -> &'static str {
        match self {
            Self::English => "English",
            Self::Spanish => "Español",
        }
    }

    /// The language a name asks for, if it is one this screen speaks.
    pub fn from_name(name: &str) -> Option<Self> {
        let wanted = name.trim().to_lowercase();
        TONGUES
            .iter()
            .copied()
            .find(|tongue| tongue.name().to_lowercase() == wanted)
    }

    /// This line, in this language.
    ///
    /// The English back, when the table has no entry — see the module note: the
    /// guard is what makes that unreachable, and a half-drawn screen is a worse
    /// answer to a missing row than one English sentence.
    pub fn say(self, english: &'static str) -> &'static str {
        let Some(table) = self.table() else {
            return english;
        };
        table
            .iter()
            .find(|(key, _)| *key == english)
            .map_or(english, |(_, said)| *said)
    }

    /// The lines this language has words for, or `None` for the one it is
    /// written in.
    ///
    /// One place, so adding a language is a table and a variant rather than a
    /// third arm in a lookup somebody has to find. The guards walk this, which
    /// is what turns "we should translate it" into a list of exactly what is
    /// missing.
    pub fn table(self) -> Option<&'static [(&'static str, &'static str)]> {
        match self {
            Self::English => None,
            Self::Spanish => Some(SPANISH),
        }
    }
}

/// Substitutes named holes into a line.
///
/// `substitute("{count} chosen", &[("count", "2")])` is `"2 chosen"`. Unknown holes
/// are left standing rather than removed, so a translation that misspells one
/// shows the misspelling instead of quietly losing the number — and the guard
/// that crosses the hole sets catches it before anybody sees either.
pub fn substitute(template: &str, holes: &[(&str, &str)]) -> String {
    // One pass over the template, not one pass per hole. Replacing them in turn
    // re-scanned what the previous one had already put in, so a value carrying
    // braces filled a hole of its own — and the values here are not ours: an
    // adapter's name, a path, a setting's accepted-values sentence, anything an
    // operator typed. `substituting_puts_the_values_where_the_holes_are` caught
    // it, which is the whole reason that assertion is in it.
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let Some(close) = rest[open..].find('}') else {
            break;
        };
        let name = &rest[open + 1..open + close];
        match holes.iter().find(|(hole, _)| *hole == name) {
            Some((_, value)) => out.push_str(value),
            // Left standing rather than removed: a number that silently
            // disappears is a sentence that reads as complete and is not.
            None => out.push_str(&rest[open..=open + close]),
        }
        rest = &rest[open + close + 1..];
    }
    out.push_str(rest);
    out
}

/// This line, in the screen's language.
macro_rules! t {
    ($tongue:expr, $english:expr) => {
        $tongue.say($english)
    };
}

/// This line, in the screen's language, with its holes filled.
macro_rules! fill {
    ($tongue:expr, $english:expr, $($name:literal => $value:expr),* $(,)?) => {
        $crate::tui::words::substitute($tongue.say($english), &[$(($name, &$value.to_string())),*])
    };
}

pub(crate) use {fill, t};

/// Where the screen's language is remembered.
///
/// `~/.estigia/screen`, beside the run pointers, and **not** in any agent's
/// contract. No agent reads which language a person's terminal is in, so a row
/// for it in the contract would be one `config set` writes, `config list` reads
/// back, and no decision consults — the defect [`crate::config::Setting::Window`]
/// records, reached from the other direction.
///
/// One line, holding the language's own name. A whole file format for one word
/// is a format somebody has to learn to fix by hand.
pub fn preference_path(home: &Path) -> PathBuf {
    home.join(".estigia").join("screen")
}

/// The same path, resolving the home the way [`remembered`] resolves it.
///
/// The fallback belongs here, beside the two functions that write and read the
/// file, and not at each caller. `forget_state` had its own copy — an
/// `options.home_dir` of `None` is the **ordinary** case in a real run and
/// `Some` only in a test — so the version that resolved nothing removed the
/// file in the suite and nothing on a machine. That was found once by running
/// the product; it survived a mutation sweep afterwards, because the fixture
/// always takes the branch that works.
///
/// One place, so there is nothing left to get wrong twice.
pub fn preference_path_for(home: Option<&Path>) -> Option<PathBuf> {
    home.map(Path::to_path_buf)
        .or_else(|| crate::paths::home_dir().ok())
        .map(|home| preference_path(&home))
}

/// The language this machine last chose, or English.
///
/// Unreadable is **English**, deliberately, and this is the one place in this
/// crate where falling back to a default is right: the declared asymmetry is
/// about guard rails, and a language is not one. Refusing to open the screen
/// because a preference file has the wrong permissions would cost somebody the
/// tool over a cosmetic answer.
pub fn remembered(home: Option<&Path>) -> Tongue {
    let Some(home) = home
        .map(Path::to_path_buf)
        .or_else(|| crate::paths::home_dir().ok())
    else {
        return Tongue::English;
    };
    std::fs::read_to_string(preference_path(&home))
        .ok()
        .and_then(|said| Tongue::from_name(&said))
        .unwrap_or_default()
}

/// Remembers this language for the next run, and says whether it stuck.
///
/// The refusal is returned rather than swallowed: the screen already changed
/// language when the key was pressed, so what is at stake is whether it is
/// *still* in that language tomorrow, and somebody who is never told will find
/// out by the screen reverting with no explanation.
pub fn remember(home: Option<&Path>, tongue: Tongue) -> Result<(), crate::outcome::Refusal> {
    use crate::outcome::{NoCommandReason, Refusal, Resolution};
    let home = home
        .map(Path::to_path_buf)
        .or_else(|| crate::paths::home_dir().ok())
        .ok_or_else(|| {
            Refusal::not_started(
                "home-not-resolvable",
                "there is no home directory to remember the screen's language in".to_owned(),
                Resolution::no_command(
                    NoCommandReason::WorldAction,
                    "a HOME or USERPROFILE the process can read",
                ),
            )
        })?;
    let path = preference_path(&home);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_refusal(&path, &error))?;
    }
    // Through the same replacement every other file goes through. One word is
    // not a reason to write in place: a run interrupted mid-write leaves a
    // truncated file, and a truncated language name reads as no preference at
    // all — the screen would come back in English having been told to be in
    // Spanish, which is the failure this whole function exists to report.
    crate::paths::replace_atomically(&path, tongue.name())
        .map_err(|error| io_refusal(&path, &error))
}

fn io_refusal(path: &Path, error: &std::io::Error) -> crate::outcome::Refusal {
    use crate::outcome::{NoCommandReason, Refusal, Resolution};
    Refusal::not_started(
        "screen-language-unwritable",
        format!(
            "the screen is in this language now, but {} could not be written ({error}), so the \
             next run opens in the old one",
            path.display()
        ),
        Resolution::no_command(
            NoCommandReason::WorldAction,
            "a writable ~/.estigia directory",
        ),
    )
}

/// Every line this screen says, in Spanish.
///
/// Ordered the way the screen is met — the menu, then setup, then the options
/// page, then the panels every page shares — rather than alphabetically, so a
/// translator reads it in the order somebody sees it and can tell whether two
/// neighbouring lines agree.
#[rustfmt::skip]
pub const SPANISH: &[(&str, &str)] = &[
    ("Repository shown", "Repositorio mostrado"),
    (
        "a checkout that answers for itself",
        "un checkout que responde por sí mismo",
    ),
    (
        "this screen only — it changes what is shown",
        "solo esta pantalla — cambia lo que se muestra",
    ),
    (
        "which checkout the rows above answer for, out of the ones that answer for themselves",
        "para qué checkout responden las filas de arriba, de entre los que responden por sí mismos",
    ),
    // The settings' own names. Shown translated and stored in English: the
    // screen says the name and the file keeps the key, the way a dropdown
    // shows a label and sends an id. The commands this screen prints still
    // carry the key, because those are lines somebody runs.
    ("Delivery authorisation", "Autorización de entrega"),
    ("Transition authorisation", "Autorización de transición"),
    ("Worktree location", "Ubicación de worktrees"),
    ("Tracker", "Gestor de issues"),
    ("Repository host", "Anfitrión del repositorio"),
    (
        "where the code lives and publications are verified — pull requests and tags",
        "dónde vive el código y se verifican las publicaciones — pull requests y etiquetas",
    ),
    ("Model routing", "Reparto de modelos"),
    ("Profile", "Perfil"),
    ("Authorisation posture", "Postura de autorización"),
    (
        "the two authorisations that can ask, at once: `auto`, `ask`, or a mix an explicit row overrides",
        "las dos autorizaciones que pueden preguntar, de una vez: `auto`, `ask`, o una mezcla que \
         una fila explícita sobrescribe",
    ),
    ("`balanced`, `cautious`, or `custom`", "`balanced`, `cautious`, o `custom`"),
    ("Model profile", "Perfil de modelos"),
    ("custom", "personalizado"),
    ("a reviewed preset that replaces the complete model route", "un preset revisado que sustituye la ruta completa de modelos"),
    ("profile: {profile}", "perfil: {profile}"),
    ("custom keeps the current route; edit the targets below to customize it", "personalizado conserva la ruta actual; edita los destinos de abajo para personalizarla"),
    ("profiles do not prove model availability or make Estigia run a model", "los perfiles no prueban la disponibilidad del modelo ni hacen que Estigia ejecute un modelo"),
    ("Integration mode", "Modo de integración"),
    ("Renewal window", "Ventana de renovación"),
    ("Git operation window", "Ventana de operaciones git"),
    ("Evidence standard", "Estándar de evidencia"),
    (
        "what a verdict here has to be backed by, and so what a reviewer may do",
        "en qué tiene que apoyarse aquí un veredicto, y por tanto qué puede hacer quien revisa",
    ),
    (
        "`reading`, or `measuring`",
        "`reading`, o `measuring`",
    ),
    ("Publication lane", "Carril de publicación"),
    (
        "whether an accepted verdict consults the publication lane for this head",
        "si un veredicto aceptado consulta el carril de publicación de esta cabeza",
    ),
    ("`on`, or `off`", "`on`, o `off`"),
    ("Unclaimed write policy", "Política de escritura sin reclamo"),
    (
        "whether a run holding no claim may write in this repository: asked each time, allowed, or refused",
        "si una ejecución sin reclamo puede escribir en este repositorio: se pregunta cada vez, se permite o se rechaza",
    ),
    (
        "`ask`, `off`, or `required`",
        "`ask`, `off` o `required`",
    ),
    ("Claim workflow", "Flujo de reclamación"),
    (
        "who witnesses a claim in this repository: Estigia writes the tracker, or the team coordinates and Estigia does not",
        "quién da fe de un reclamo en este repositorio: Estigia escribe en el tracker, o el equipo coordina y Estigia no",
    ),
    ("`autonomous`, or `assisted`", "`autonomous` o `assisted`"),
    ("Review budget", "Presupuesto de revisión"),
    (
        "the per-context token figure a review is reported against, and nothing more",
        "la cifra de tokens por contexto con la que se compara una revisión, y nada más",
    ),
    (
        "`none`, or a token count such as `150000`",
        "`none`, o un recuento de tokens, como `150000`",
    ),
    ("Change size", "Tamaño del cambio"),
    (
        "how many changed lines a pull request aims to stay under",
        "cuántas líneas cambiadas trata de no pasar un pull request",
    ),
    (
        "a number of lines, such as `800`",
        "un número de líneas, como `800`",
    ),
    ("Corrections per lineage", "Correcciones por linaje"),
    (
        "how many corrections one review lineage may spend before a person decides",
        "cuántas correcciones puede gastar un linaje de revisión antes de que decida una persona",
    ),
    (
        "a positive whole number, such as `3`",
        "un número entero positivo, como `3`",
    ),
    ("Irreversible commands", "Órdenes irreversibles"),
    ("Summary language", "Idioma del resumen"),
    ("Issue body language", "Idioma del cuerpo del issue"),
    ("Loop plugin", "Plugin de bucle"),
    (
        "whether OpenCode gets the plugin, and Codex the Stop and SessionStart hooks, that continue a session while the run still holds its issue or another is claimable",
        "si OpenCode recibe el plugin, y Codex los hooks Stop y SessionStart, que continúan una sesión mientras la ejecución sigue teniendo su issue o hay otro reclamable",
    ),
    ("Goal loop plugin", "Plugin de objetivo y bucle"),
    (
        "whether OpenCode also gets the generic plugin that offers `/goal` and `/loop`, which consults no tracker and runs no Estigia command",
        "si OpenCode recibe además el plugin genérico que ofrece `/goal` y `/loop`, que no consulta ningún tracker ni ejecuta ninguna orden de Estigia",
    ),
    // The groups, and the one-word scope every row carries beside its value.
    ("Authority", "Autoridad"),
    ("Repository & tracker", "Repositorio y tracker"),
    ("Gate limits", "Límites de la puerta"),
    ("Review & change", "Revisión y cambio"),
    ("Agent runtime", "Ejecución del agente"),
    ("This machine", "Esta máquina"),
    ("agent", "agente"),
    ("everywhere", "en todas partes"),
    ("repository", "repositorio"),
    ("machine", "máquina"),
    ("a repository may tighten", "un repositorio puede endurecerlo"),
    ("choose agents, and open each one's own settings", "elige agentes y abre los ajustes propios de cada uno"),
    ("Options", "Opciones"),
    ("the settings that are the same whichever agent asks", "los ajustes que son iguales lo pregunte el agente que lo pregunte"),
    ("Push guard", "Guardia de push"),
    ("the pre-push hook: the one boundary no agent can go around", "el hook de pre-push: el único límite que ningún agente puede rodear"),
    ("Doctor", "Diagnóstico"),
    ("check that everything a run needs before it swears actually works", "comprueba que todo lo que una ejecución necesita antes de jurar funciona de verdad"),
    ("Help", "Ayuda"),
    ("the keys, and what this screen is", "las teclas, y qué es esta pantalla"),
    ("Quit", "Salir"),
    ("leave — nothing is written that was not already installed", "salir — no se escribe nada que no estuviera ya instalado"),
    ("ACTIONS", "ACCIONES"),
    ("Menu", "Menú"),
    ("Edit", "Edición"),
    ("Agents", "Agentes"),
    ("editing {agent}'s own table", "editando la tabla propia de {agent}"),
    ("editing the shared contract", "editando el contrato compartido"),
    ("{chosen} of {known} agents chosen", "{chosen} de {known} agentes elegidos"),
    ("no agent is chosen, so there is nowhere to write these", "no hay ningún agente elegido, así que esto no tiene dónde escribirse"),
    ("one answer, into each of the {count} chosen agents", "una respuesta, en cada uno de los {count} agentes elegidos"),
    ("one answer, into the one chosen agent", "una respuesta, en el único agente elegido"),
    ("CONTRACT", "CONTRATO"),
    ("AGENTS", "AGENTES"),
    ("All chosen agents", "Todos los agentes elegidos"),
    ("ALL INSTALLED AGENTS", "TODOS LOS AGENTES INSTALADOS"),
    ("answers here are written to every chosen agent at once", "las respuestas de aquí se escriben en todos los agentes elegidos a la vez"),
    ("chosen — enter opens its own settings", "elegido — intro abre sus ajustes propios"),
    ("install {agent}", "instalar {agent}"),
    ("changed in {agent}: {named}", "cambiado en {agent}: {named}"),
    ("the shared contract", "el contrato compartido"),
    ("Review", "Revisión"),
    ("nothing is written until you confirm", "no se escribe nada hasta que confirmes"),
    ("⏎ write it", "⏎ lo escribe"),
    ("gated", "con puerta"),
    ("contract only", "solo contrato"),
    ("installed", "instalado"),
    ("installed — will be left alone", "instalado — se queda como está"),
    ("will be installed", "se instalará"),
    ("not chosen — space ticks it", "no elegido — espacio lo marca"),
    ("Estigia gates this agent's tool calls: every write goes through the claim", "Estigia pone puerta a las llamadas de herramienta de este agente: toda escritura pasa por la reclamación"),
    ("Estigia cannot gate this agent's tool calls: it gets the contract and the pre-push guard, and its authorisations are asked for rather than held", "Estigia no puede poner puerta a las llamadas de este agente: recibe el contrato y la guardia de pre-push, y sus autorizaciones se piden en vez de sostenerse"),
    ("OPTIONS", "OPCIONES"),
    ("→ open", "→ entrar"),
    ("← up", "← subir"),
    ("n new folder", "n carpeta nueva"),
    ("⏎ make it", "⏎ crearla"),
    (
        "a folder name, not a path: no separators, and not `.` or `..`",
        "un nombre de carpeta, no una ruta: sin separadores, y ni `.` ni `..`",
    ),
    ("REPOSITORY OPTIONS", "OPCIONES DE REPOSITORIO"),
    ("Interface language", "Idioma de la interfaz"),
    ("the language this screen speaks: this machine only, never a contract", "el idioma en que habla esta pantalla: solo esta máquina, nunca un contrato"),
    ("applied at once, and remembered in ~/.estigia/screen", "se aplica al momento, y se recuerda en ~/.estigia/screen"),
    ("one of the languages this screen has words for", "uno de los idiomas para los que esta pantalla tiene palabras"),
    ("Done", "Hecho"),
    ("Not done", "No hecho"),
    ("any key closes this", "cualquier tecla lo cierra"),
    ("nothing to show", "nada que mostrar"),
    ("{title} — the end", "{title} — el final"),
    ("{title} — {more} more below", "{title} — {more} más abajo"),
    ("the checks did not finish", "las comprobaciones no terminaron"),
    ("{agent} model catalog did not finish loading. Type a model ID instead.", "el catálogo de modelos de {agent} no terminó de cargar. Escribe un ID de modelo."),
    ("running the checks — the skill, the tracker CLI, the guard and the remote…", "ejecutando las comprobaciones — el skill, la CLI del tracker, la guardia y el remoto…"),
    ("push guard installed in {where}, and read back as installed", "guardia de push instalada en {where}, y leída de vuelta como instalada"),
    ("push guard chained in {where}: a hook of yours already hands the push to Estigia, so nothing was written", "guardia de push encadenada en {where}: un hook tuyo ya entrega el push a Estigia, así que no se escribió nada"),
    ("created folder {path} on disk now; this is not held back until s", "carpeta {path} creada en disco ahora; esto no espera a s"),
    ("nothing changed — there is nothing to save", "nada ha cambiado — no hay nada que guardar"),
    ("unsaved changes — press again to discard, or s to save", "cambios sin guardar — pulsa otra vez para descartarlos, o s para guardar"),
    ("could not work out where this executable is, so the hook would name nothing", "no se pudo averiguar dónde está este ejecutable, así que el hook no nombraría nada"),
    ("choose at least one agent — space ticks the one under the cursor", "elige al menos un agente — espacio marca el que está bajo el cursor"),
    ("unsaved changes — press again to discard, or s to install", "cambios sin guardar — pulsa otra vez para descartarlos, o s para instalar"),
    ("{label} restored to what is installed", "{label} restaurado a lo que está instalado"),
    ("{target} restored to what is installed", "{target} restaurado a lo que está instalado"),
    ("Model profile restored to what is installed", "Perfil de modelos restaurado a lo que está instalado"),
    ("{label} is already remembered — there is nothing unsaved to restore", "{label} ya está recordado — no hay nada sin guardar que restaurar"),
    ("type a value…", "escribe un valor…"),
    ("Models for {target}", "Modelos para {target}"),
    ("MODELS", "MODELOS"),
    ("PHASES", "FASES"),
    ("ROLES", "ROLES DELEGADOS"),
    ("type a model ID…", "escribe un ID de modelo…"),
    ("inherit", "heredar"),
    ("different values", "valores diferentes"),
    ("assignment: {model}", "asignación: {model}"),
    ("model declared for orchestration", "modelo declarado para la orquestación"),
    ("model declared for applying changes", "modelo declarado para aplicar cambios"),
    ("model declared for this delegated agent", "modelo declarado para este agente delegado"),
    ("a model ID must fit one key=model entry: no comma, semicolon, pipe, or line break", "un ID de modelo debe caber en una entrada key=model: sin coma, punto y coma, barra vertical ni salto de línea"),
    ("accepts: any model ID that fits one key=model entry; no comma, semicolon, pipe, or line break; catalogs are advisory", "acepta: cualquier ID de modelo que quepa en una entrada key=model; sin coma, punto y coma, barra vertical ni salto de línea; los catálogos son orientativos"),
    ("{agent} model suggestions are advisory; Estigia neither validates nor runs models", "las sugerencias de modelos de {agent} son orientativas; Estigia no valida ni ejecuta modelos"),
    ("loaded from `opencode models` without refresh; advisory only", "cargado desde `opencode models` sin refrescar; solo orientativo"),
    ("loading OpenCode's model catalog…", "cargando el catálogo de modelos de OpenCode…"),
    ("OpenCode model catalog unavailable or empty; type a model ID", "el catálogo de modelos de OpenCode no está disponible o está vacío; escribe un ID de modelo"),
    ("no verified model catalog for {agent}; type a model ID", "no hay un catálogo de modelos verificado para {agent}; escribe un ID de modelo"),
    ("shared answers have no single agent model catalog; type a model ID", "las respuestas compartidas no tienen el catálogo de un único agente; escribe un ID de modelo"),
    ("Model routing differs across selected agents; unify it or edit each agent", "El Reparto de modelos difiere entre los agentes seleccionados; unifícalo o edita cada agente"),
    ("naming this role in `Model routing` is what installs its definition; removing the entry retracts it, and the model beside it says only what it runs on", "nombrar este rol en `Model routing` es lo que instala su definición; quitar la entrada la retira, y el modelo de al lado solo dice con qué corre"),
    ("{target} is a routing declaration, not proof that a host executes it", "{target} es una declaración de reparto, no una prueba de que un anfitrión lo ejecute"),
    ("{model} is a {hosts} model; {agents} does not list it, and it is stored as given", "{model} es un modelo de {hosts}; {agents} no lo lista, y se guarda tal cual"),
    ("loading the {agent} model catalog…", "cargando el catálogo de modelos de {agent}…"),
    ("{agent} model catalog unavailable: {why}. Type a model ID instead.", "el catálogo de modelos de {agent} no está disponible: {why}. Escribe un ID de modelo en su lugar."),
    ("accepts: {accepted}", "acepta: {accepted}"),
    ("type to change", "escribe para cambiar"),
    ("this agent only ({agent}) — installed: {installed}", "solo este agente ({agent}) — instalado: {installed}"),
    ("every selected agent — installed: {installed}", "cada agente seleccionado — instalado: {installed}"),
    ("every agent — installed: {installed}", "todos los agentes — instalado: {installed}"),
    (
        "this repository — installed: {installed}",
        "este repositorio — instalado: {installed}",
    ),
    (
        "this machine, every repository — installed: {installed}",
        "esta máquina, todos los repositorios — instalado: {installed}",
    ),
    ("(differs by agent)", "(difiere por agente)"),
    ("not held", "(sin puerta)"),
    ("Estigia does not gate this agent's tool calls — the contract asks, and the pre-push guard still holds the push", "Estigia no pone puerta a las llamadas de herramienta de este agente — el contrato lo pide, y la guardia de pre-push sigue sosteniendo el push"),
    ("whether this agent may deliver a reviewed change, or has to ask", "si este agente puede entregar un cambio revisado, o tiene que pedirlo"),
    ("whether this agent may move a task between states on its own", "si este agente puede mover una tarea entre estados por su cuenta"),
    ("where isolated checkouts are made, when a run needs one", "dónde se hacen los checkouts aislados, cuando una ejecución necesita uno"),
    ("where this repository's issues live — the claim is adjudicated there", "dónde viven los issues de este repositorio — la reclamación se adjudica ahí"),
    ("which model, and at what effort, each role runs on, for this agent — and which roles this agent has a definition for", "en qué modelo, y con qué esfuerzo, corre cada rol, para este agente — y para qué roles tiene definición este agente"),
    ("whether work integrates through branches or straight onto trunk", "si el trabajo se integra por ramas o directo sobre trunk"),
    ("how long a routine write may ride on the last verification", "cuánto puede una escritura rutinaria apoyarse en la última verificación"),
    ("the daily range commits, pushes, and branch creation may run in", "el rango diario en que pueden correr los commits, los pushes y la creación de ramas"),
    ("the commands this repository treats as one-way doors", "los comandos que este repositorio trata como puertas de un solo sentido"),
    ("the language the summary sentence at the top of an issue is in", "el idioma de la frase-resumen de la cabecera de un issue"),
    ("the language the rest of an issue body is written in", "el idioma en que se escribe el resto del cuerpo de un issue"),
    ("`auto`, `ask`, or `ask` with a duration such as `ask 30m`", "`auto`, `ask`, o `ask` con una duración como `ask 30m`"),
    ("`unset`, or an absolute directory", "`unset`, o un directorio absoluto"),
    ("`github` or `github <owner>/<name>`", "`github` o `github <dueño>/<nombre>`"),
    ("`github`, `github <owner>/<name>`, or `linear <team-key>` under assisted work", "`github`, `github <dueño>/<nombre>`, o `linear <clave-de-equipo>` en trabajo asistido"),
    ("`unset`, `by task`, or one cell holding two lists separated by `;`. The first is comma-separated key=model pairs, as in `orchestrate=fable, apply=sonnet`; the second, after the `;`, is comma-separated key=effort pairs, as in `apply=low`. `by task` says the shape of the work decides the model and the effort and the launcher records what it chose, instead of a role's name. A key is a role (developer, analyst, reviewer, judge), orchestrate, or apply. A model ID may use any catalog spelling, or inherit, but no comma, semicolon, pipe, or line break. An effort is low, medium, high, xhigh or max. Naming a role installs its definition; reviewer and judge install nothing", "`unset`, `by task`, o una celda con dos listas separadas por `;`. La primera son pares clave=modelo separados por comas, como `orchestrate=fable, apply=sonnet`; la segunda, tras el `;`, son pares clave=esfuerzo separados por comas, como `apply=low`. `by task` dice que la forma del trabajo decide el modelo y el esfuerzo y quien lanza registra lo que eligió, en lugar del nombre de un rol. Una clave es un rol (developer, analyst, reviewer, judge), orchestrate o apply. Un ID de modelo puede usar cualquier nombre de catálogo, o inherit, pero no coma, punto y coma, barra vertical ni salto de línea. Un esfuerzo es low, medium, high, xhigh o max. Nombrar un rol instala su definición; reviewer y judge no instalan nada"),
    ("`branch`, or `trunk`", "`branch`, o `trunk`"),
    ("`default`, or a shorter duration such as `30s` or `1m`", "`default`, o una duración más corta como `30s` o `1m`"),
    ("`disabled`, `enabled 09:00-17:00`, `enabled 22:00-06:00 inverted`, or `enabled mon-fri 08:00-17:00 inverted`", "`disabled`, `enabled 09:00-17:00`, `enabled 22:00-06:00 inverted`, o `enabled mon-fri 08:00-17:00 inverted`"),
    ("`none`, or commands separated by commas such as `npm publish`", "`none`, o comandos separados por comas como `npm publish`"),
    ("a plain language name such as `English`", "un nombre de idioma llano, como `English`"),
    ("up/down move", "arriba/abajo mueve"),
    ("enter choose", "intro elige"),
    ("? help", "? ayuda"),
    ("enter open", "intro abre"),
    ("s apply", "s aplica"),
    ("⏎ / space all answers", "⏎ / espacio todas las respuestas"),
    ("q quit", "q salir"),
    ("any key returns", "cualquier tecla vuelve"),
    ("↑↓ scroll", "↑↓ desplaza"),
    ("↑↓ move", "↑↓ mueve"),
    ("←→ change", "←→ cambia"),
    ("←→ effort", "←→ esfuerzo"),
    ("⏎ apply", "⏎ aplica"),
    ("⏎ / space choose", "⏎ / espacio elige"),
    ("Esc cancel", "Esc cancela"),
    ("Esc back", "Esc atrás"),
    ("⌫ delete", "⌫ borra"),
    ("u uninstall", "u desinstala"),
    ("inert", "inerte"),
    ("unreadable", "ilegible"),
    ("⏎ remove it", "⏎ lo quita"),
    ("the push guard", "el guard de push"),
    ("uninstall {what}", "desinstalar {what}"),
    ("Uninstall", "Desinstalar"),
    ("nothing is removed until you confirm", "no se quita nada hasta que confirmes"),
    ("nothing of Estigia's to remove from {agent}", "no hay nada de Estigia que quitar de {agent}"),
    ("read back: still installed", "releído: sigue instalado"),
    ("read back: its block is gone, but the uninstall did not finish", "releído: su bloque ya no está, pero la desinstalación no terminó"),
    ("read back: could not be read ({reason})", "releído: no se pudo leer ({reason})"),
    ("{agent} uninstalled: {count} file(s) changed, read back as not installed", "{agent} desinstalado: {count} archivo(s) cambiados, releído como no instalado"),
    ("push guard removed from {where}", "guard de push quitado de {where}"),
    ("push guard would be removed from {where}", "el guard de push se quitaría de {where}"),
    ("no push guard in {where} — nothing to remove", "no hay guard de push en {where} — nada que quitar"),
    ("the hook in {where} is not only Estigia's and is left alone — nothing was removed", "el hook de {where} no es solo de Estigia y se deja como está — no se quitó nada"),
    ("space tick", "espacio marca"),
    ("r restore", "r restaura"),
    // The help page: a page rather than a line, and last for that reason.
    (crate::tui::HELP, HELP_ES),
];

#[cfg(test)]
mod tests;
