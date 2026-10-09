//! The five emails, in English and Brazilian Portuguese. Plain text, no
//! links other than the site's own pages, no tracking. Subjects never
//! carry the code or the invite: notification previews show subjects.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Locale {
    En,
    PtBr,
}

impl Locale {
    /// As the site sends it in `signup/start`.
    pub fn parse(raw: &str) -> Option<Self> {
        match raw {
            "en" => Some(Self::En),
            "pt-BR" | "pt-br" => Some(Self::PtBr),
            _ => None,
        }
    }

    /// As stored on `accounts.locale`; admin-created accounts have none.
    pub fn from_db(stored: Option<&str>) -> Self {
        stored.and_then(Self::parse).unwrap_or(Self::En)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::PtBr => "pt-BR",
        }
    }
}

const DOWNLOAD_URL: &str = "https://havenkeys.net/download";
const PRICING_URL: &str = "https://havenkeys.net/pricing";

pub fn signup_code(locale: Locale, code: &str) -> (String, String) {
    match locale {
        Locale::En => (
            "Your HavenKeys sign-up code".into(),
            format!(
                "Your HavenKeys sign-up code is:\n\n{code}\n\n\
                 It is valid for 15 minutes. If you did not ask for it, ignore this message; \
                 nothing happens without the code.\n"
            ),
        ),
        Locale::PtBr => (
            "Seu código de cadastro no HavenKeys".into(),
            format!(
                "Seu código de cadastro no HavenKeys é:\n\n{code}\n\n\
                 Ele vale por 15 minutos. Se você não pediu este código, ignore esta mensagem; \
                 nada acontece sem ele.\n"
            ),
        ),
    }
}

pub fn signup_invite(locale: Locale, invite: &str) -> (String, String) {
    match locale {
        Locale::En => (
            "Finish setting up HavenKeys".into(),
            format!(
                "Your HavenKeys account is ready to be set up.\n\n\
                 Install HavenKeys from {DOWNLOAD_URL}, open it, choose \"I have a setup code\" \
                 and paste this code:\n\n{invite}\n\n\
                 It is valid for 24 hours and works once. Whoever has this code and this mailbox \
                 can set up the account, so do not forward this message.\n"
            ),
        ),
        Locale::PtBr => (
            "Conclua a configuração do HavenKeys".into(),
            format!(
                "Sua conta HavenKeys está pronta para ser configurada.\n\n\
                 Instale o HavenKeys em {DOWNLOAD_URL}, abra o app, escolha \"Tenho um código de \
                 configuração\" e cole este código:\n\n{invite}\n\n\
                 Ele vale por 24 horas e funciona uma única vez. Quem tiver este código e esta \
                 caixa de e-mail consegue configurar a conta, então não encaminhe esta mensagem.\n"
            ),
        ),
    }
}

pub fn already_registered(locale: Locale) -> (String, String) {
    match locale {
        Locale::En => (
            "You already have a HavenKeys account".into(),
            "Someone asked to create a HavenKeys account with this address, but it already has \
             one.\n\nSign in from the HavenKeys app with your email, master password and Secret \
             Key. If this was not you, nothing has changed and you can ignore this message.\n"
                .into(),
        ),
        Locale::PtBr => (
            "Você já tem uma conta HavenKeys".into(),
            "Alguém pediu para criar uma conta HavenKeys com este endereço, mas ele já tem \
             uma.\n\nEntre pelo app HavenKeys com seu e-mail, senha mestra e Secret Key. Se não \
             foi você, nada mudou e você pode ignorar esta mensagem.\n"
                .into(),
        ),
    }
}

pub fn trial_ending(locale: Locale, days: i64) -> (String, String) {
    match locale {
        Locale::En => (
            format!("Your HavenKeys trial ends in {days} days"),
            format!(
                "Your free trial of HavenKeys ends in {days} days.\n\n\
                 After that, your vault stays readable on every device and you can export it \
                 at any time, but changes and autofill stop until you subscribe.\n\n\
                 Plans: {PRICING_URL}\n"
            ),
        ),
        Locale::PtBr => (
            format!(
                "Seu período de teste do HavenKeys termina em {days} {}",
                if days == 1 { "dia" } else { "dias" }
            ),
            format!(
                "Seu período de teste gratuito do HavenKeys termina em {days} {}.\n\n\
                 Depois disso, seu cofre continua legível em todos os dispositivos e você pode \
                 exportá-lo quando quiser, mas alterações e preenchimento automático param até \
                 você assinar.\n\nPlanos: {PRICING_URL}\n",
                if days == 1 { "dia" } else { "dias" }
            ),
        ),
    }
}

pub fn trial_ended(locale: Locale) -> (String, String) {
    match locale {
        Locale::En => (
            "Your HavenKeys trial has ended".into(),
            format!(
                "Your free trial of HavenKeys has ended.\n\n\
                 Your vault is still readable on every device and you can export it at any time. \
                 Changes and autofill are paused until you subscribe.\n\nPlans: {PRICING_URL}\n"
            ),
        ),
        Locale::PtBr => (
            "Seu período de teste do HavenKeys terminou".into(),
            format!(
                "Seu período de teste gratuito do HavenKeys terminou.\n\n\
                 Seu cofre continua legível em todos os dispositivos e você pode exportá-lo \
                 quando quiser. Alterações e preenchimento automático ficam pausados até você \
                 assinar.\n\nPlanos: {PRICING_URL}\n"
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn locales_parse_as_the_site_sends_them() {
        assert_eq!(Locale::parse("en"), Some(Locale::En));
        assert_eq!(Locale::parse("pt-BR"), Some(Locale::PtBr));
        assert_eq!(Locale::parse("pt-br"), Some(Locale::PtBr));
        assert_eq!(Locale::parse("fr"), None);
        assert_eq!(Locale::from_db(None), Locale::En);
        assert_eq!(Locale::from_db(Some("pt-BR")), Locale::PtBr);
    }

    #[test]
    fn the_code_and_the_invite_stand_on_their_own_line() {
        for locale in [Locale::En, Locale::PtBr] {
            let (subject, body) = signup_code(locale, "123456");
            assert!(!subject.contains("123456"), "codes stay out of subjects");
            assert!(body.lines().any(|l| l.trim() == "123456"), "{body}");
            let (_, body) = signup_invite(locale, "HKINV1-abc");
            assert!(body.lines().any(|l| l.trim() == "HKINV1-abc"));
        }
    }

    #[test]
    fn every_email_exists_in_both_languages_and_is_plain_text() {
        for locale in [Locale::En, Locale::PtBr] {
            for (subject, body) in [
                signup_code(locale, "000000"),
                signup_invite(locale, "HKINV1-x"),
                already_registered(locale),
                trial_ending(locale, 3),
                trial_ended(locale),
            ] {
                assert!(!subject.is_empty() && !body.is_empty());
                assert!(!body.contains('<'), "no HTML: {body}");
                assert!(!body.contains("utm_"), "no tracking: {body}");
            }
        }
        assert!(trial_ending(Locale::En, 3).0.contains("3 days"));
        assert!(trial_ending(Locale::PtBr, 1).0.contains("1 dia"));
    }
}
