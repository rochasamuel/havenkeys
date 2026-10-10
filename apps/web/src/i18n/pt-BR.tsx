import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import { Icon } from "../components/Icon";
import type { Messages } from "./en";

/*
 * Português do Brasil. Same shape as en.tsx, enforced by the Messages type.
 * Secret Key and havenkeys-server keep their English names: they are what
 * the app itself shows. Recovery Sheet is "Folha de Recuperação", as in the app.
 */

const GH = "https://github.com/rochasamuel/havenkeys";
const DOCS = `${GH}/blob/main/docs/`;
const CHROME_STORE = "https://chromewebstore.google.com/detail/havenkeys/fmmfkakdkkcfpdnfmbngnlelbfaogafo";
const FIREFOX_STORE = "https://addons.mozilla.org/firefox/addon/havenkeys/";

function Ext({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noreferrer">
      {children}
    </a>
  );
}

export const ptBR: Messages = {
  meta: {
    title: "HavenKeys — senhas que só você abre",
    description:
      "O HavenKeys é um gerenciador de senhas gratuito e de código aberto para seu computador, celular e navegador. Seu cofre fica trancado nos seus dispositivos; ninguém mais consegue abrir.",
  },

  common: {
    disclaimer:
      "Este software não passou por uma auditoria de segurança independente e não deve ser considerado um substituto para gerenciadores de senhas auditados profissionalmente em uso de produção de alto valor.",
    downloadCta: "Baixar o HavenKeys",
    addToChrome: "Adicionar ao Chrome",
    addToFirefox: "Adicionar ao Firefox",
    createAccount: "Criar conta",
    beta: "Beta",
    betaNotice:
      "O HavenKeys está em beta. Funciona e é o que usamos todo dia, mas espere arestas, e guarde bem sua Folha de Recuperação.",
    installerWarning:
      "Os instaladores ainda não são assinados, então o Windows SmartScreen e o Gatekeeper do macOS vão avisar na primeira execução. No Android, o APK tem a assinatura verificada pelo próprio sistema.",
  },

  nav: {
    homeAria: "Início do HavenKeys (beta)",
    mainAria: "Principal",
    security: "Segurança",
    selfHost: "Auto-hospedar",
    developers: "Desenvolvedores",
    pricing: "Planos",
    githubAria: "HavenKeys no GitHub",
    download: "Baixar",
    switchShort: "EN",
    switchAria: "View the site in English",
  },

  footer: {
    tagline: "Senhas que só você abre.",
    aria: "Rodapé",
    download: "Baixar",
    security: "Segurança",
    selfHost: "Auto-hospedar",
    developers: "Desenvolvedores",
    pricing: "Planos",
    github: "GitHub",
    privacy: "Privacidade",
    terms: "Termos e licença",
    deleteAccount: "Excluir conta",
    license: "Apache-2.0",
    languageAria: "Idioma",
  },

  home: {
    heroTitle: (
      <>
        Suas senhas, trancadas — e só <em>suas</em>.
      </>
    ),
    heroLede:
      "Um gerenciador de senhas para o seu computador, celular e navegador. Seu cofre fica trancado nos seus próprios dispositivos, e ninguém mais — nem o servidor que guarda a sua cópia — consegue abri-lo.",
    heroMeta: "Gratuito e open source · Windows, macOS, Linux e Android · Chrome e Firefox",
    desktopAlt:
      "O app de desktop do HavenKeys: uma barra lateral com as seções do cofre, uma lista de logins e o login da Fernway aberto com o usuário, a senha oculta e um código de uso único ao vivo.",
    popupAlt: "O popup do HavenKeys no navegador: dois logins salvos da Fernway, um botão Fill e um código de uso único.",
    menuAlt: "O menu do HavenKeys na página oferecendo dois logins salvos.",

    cardsTitle: "Tudo o que você precisa. Nada que não precisa.",
    cardsLede: "O essencial de um gerenciador de senhas moderno, feito com cuidado.",
    cards: [
      { icon: "key", title: "Preenche quando você clica", text: "Na página de login, o HavenKeys oferece o que você salvou para aquele site e espera você escolher." },
      { icon: "lock", title: "Passkeys", text: "Crie passkeys e entre com elas, no computador e no celular." },
      { icon: "check", title: "Códigos de verificação inclusos", text: "Os códigos de duas etapas ficam junto da senha. Sem app autenticador à parte." },
      { icon: "laptop", title: "Funciona offline", text: "Cada dispositivo guarda sua própria cópia trancada, então suas senhas estão lá mesmo sem internet." },
      { icon: "monitor", title: "Em todos os seus dispositivos", text: "App de desktop, app Android e extensão para Chrome e Firefox, sempre sincronizados." },
      { icon: "download", title: "Traga suas senhas", text: "Importe do 1Password, Bitwarden, LastPass, KeePassXC, Chrome ou Firefox. Exporte quando quiser." },
    ],

    trustTitle: "Ninguém mais consegue abrir.",
    trustBody:
      "Seu cofre é trancado no seu dispositivo com duas coisas que só você tem: sua senha mestra e uma Secret Key que o HavenKeys cria para você. O que chega ao servidor já vai trancado, então quem o opera — nós incluídos — não consegue ler nenhuma senha.",
    trustLink: "Como funciona, em detalhes",

    stepsTitle: "Comece em cinco passos.",
    steps: [
      { title: "Instale o app", text: "Baixe o HavenKeys para Windows, macOS, Linux ou Android." },
      { title: "Tenha sua conta", text: "Crie uma conta no nosso servidor, ou rode o seu." },
      { title: "Imprima sua Folha de Recuperação", text: "Ela guarda sua Secret Key. Deixe-a em lugar seguro: é com ela que você entra num dispositivo novo." },
      { title: "Traga suas senhas", text: "Importe do seu gerenciador antigo, ou adicione aos poucos: o HavenKeys oferece para salvar as novas." },
      { title: "Adicione a extensão e clique", text: "Instale no Chrome ou no Firefox. Numa página de login, clique no campo e escolha seu login." },
    ],

    selfHostTitle: "Prefere seu próprio servidor?",
    selfHostBody:
      "Rode o HavenKeys num pequeno servidor seu ou no Railway. A instalação leva poucos comandos, e a instalação com Docker inclui backups diários.",
    selfHostCta: "Rode seu próprio servidor",

    closerTitle: (
      <>
        Leve suas senhas para <em>casa</em>.
      </>
    ),
    closerLede: "Instale o app, crie uma conta e adicione a extensão.",
    readSource: "Ler o código-fonte",
  },

  journey: {
    chapters: [
      {
        label: "Digitada",
        title: (
          <>
            Você digita <em>uma vez</em>.
          </>
        ),
        body: [
          "O desbloqueio começa com dois segredos. A senha mestra nunca vira uma chave por si só: ela passa pelo Argon2id, que é lento e consome muita memória de propósito, para que cada tentativa custe hardware de verdade a um atacante.",
          "O resultado é combinado com a sua Secret Key, 128 bits aleatórios criados no seu dispositivo. Juntos, eles abrem a chave do cofre, e tudo isso acontece em Rust, dentro do app de desktop.",
        ],
      },
      {
        label: "Selada",
        title: (
          <>
            Ela é selada <em>antes</em> de ser salva.
          </>
        ),
        body: [
          "Cada item é cifrado no seu dispositivo com AES-256-GCM. Isso inclui o título, o usuário e o site, não só a senha, então quem lê o banco de dados não descobre quais sites você usa.",
          "Cada gravação recebe um nonce novo, e cada bloco fica vinculado ao seu cofre e ao seu item. Mova um para qualquer outro lugar e ele se recusa a abrir.",
        ],
      },
      {
        label: "Guardada",
        title: (
          <>
            Seu servidor guarda. Ele <em>não consegue</em> ler.
          </>
        ),
        body: [
          <>
            O HavenKeys sincroniza pelo <code>havenkeys-server</code>, um servidor pequeno que você
            mesmo roda, ou o nosso, se você criar uma conta. Quem o roda guarda só dados
            trancados que não consegue abrir.
          </>,
          "O servidor é o único lugar onde as alterações são gravadas. Cada computador mantém sua própria cópia cifrada, então desbloqueio, busca, códigos de uso único e preenchimento continuam funcionando offline.",
        ],
      },
      {
        label: "Preenchida",
        title: (
          <>
            Ela volta quando <em>você</em> pede.
          </>
        ),
        body: [
          "Clique em um campo de login e o HavenKeys oferece os logins salvos para aquele site. Nada é preenchido ao carregar a página, e nada é preenchido até você escolher.",
          "O app de desktop confere o endereço da página com os sites salvos no item antes de enviar qualquer coisa, então um domínio parecido não recebe nada, diga o que disser a página ou a extensão.",
        ],
      },
    ],
    stageFoot: "Um login, acompanhado do teclado ao preenchimento · dados de demonstração",
  },

  scenes: {
    keys: {
      masterLabel: "Senha mestra",
      masterWhere: "Só na sua cabeça",
      secretLabel: "Secret Key",
      secretWhere: "Nos seus dispositivos e na sua Folha de Recuperação",
      argonParam: "128 MiB de memória · 4 passadas · 4 faixas",
      hkdfParam: "chave mestra + Secret Key → chave de cifragem de chaves",
      vaultKey: "Chave do cofre",
      vaultParam: "256 bits aleatórios, abertos na memória, apagados quando você tranca",
    },
    seal: {
      labels: ["Título", "Usuário", "Senha", "Site", "Código de uso único"],
      totpPlain: "configurado",
      plaintext: "Texto puro, na memória",
      sealed: "Selado",
      notes: ["AES-256-GCM", "nonce novo de 96 bits a cada gravação", "vinculado ao cofre, ao item e à função"],
    },
    store: {
      where: "num servidor seu ou nosso",
      blobsAria: "O que o servidor guarda",
      cantOpen: "Não abre títulos, usuários, sites, senhas, códigos nem notas",
      doesSee: "Vê o seu e-mail, quantos itens existem, o tamanho deles e quando mudaram",
      laptop: "Notebook",
      desktop: "Desktop",
      deviceNote: "cópia cifrada, funciona offline",
    },
    fill: {
      emailLabel: "E-mail",
      menuAlt: "O menu de sugestões do HavenKeys abaixo de um campo de e-mail, com dois logins salvos da Fernway.",
      origins: [
        "Salvo aqui. Escolha e ele preenche.",
        "Mesmo site, então é oferecido.",
        "Negado. Outro site.",
        "Negado. Parece, mas não é.",
      ],
    },
  },

  showcase: {
    tabsAria: "Cenários da extensão de navegador",
    scenarios: {
      signin: {
        tab: "Entrar",
        title: "Só os logins salvos para este site.",
        body: "Clique no campo de e-mail e os logins salvos para este site aparecem logo abaixo. Escolha um e os dois campos são preenchidos. O menu roda no quadro da própria extensão, onde a página não consegue ler nem controlar.",
        alt: "Menu do HavenKeys listando dois logins da Fernway",
      },
      otp: {
        tab: "Código de uso único",
        title: "O código, nunca o segredo.",
        body: "O app de desktop calcula os seis dígitos e envia só eles. O segredo TOTP fica no cofre, então nem a página nem a extensão chegam a vê-lo.",
        alt: "Menu do HavenKeys oferecendo preencher o código de uso único",
      },
      signup: {
        tab: "Nova conta",
        title: "Uma senha forte em um clique.",
        body: "Em um formulário de cadastro, o HavenKeys oferece uma senha gerada. O app de desktop a cria com a fonte segura de aleatoriedade do seu sistema operacional e preenche todos os campos de nova senha.",
        alt: "Menu do HavenKeys oferecendo gerar uma senha forte",
      },
      save: {
        tab: "Salvar",
        title: "Ele pergunta. Nunca presume.",
        body: "Depois que você entra com algo novo, o HavenKeys oferece salvar ou atualizar a senha antiga. Ele só oferece o que você digitou, então uma página não consegue usar o aviso para pescar senhas.",
        alt: "O HavenKeys perguntando se deve salvar o novo login da Fernway",
      },
      popup: {
        tab: "Barra de ferramentas",
        title: "Funciona sem o menu na página também.",
        body: "Prefere sem menu abaixo dos campos de login? Desative as sugestões na página nas opções da extensão: o botão da barra de ferramentas preenche a aba atual com as mesmas verificações de origem, e as ofertas para salvar logins e as chaves de acesso continuam funcionando.",
        alt: "O popup do HavenKeys na barra de ferramentas com dois logins, botões Fill e um código de uso único",
      },
    },
    soloCaption: (url: ReactNode) => <>Em {url} · site de demonstração</>,
    demo: {
      badge: "Site de demonstração",
      signIn: "Entrar",
      welcomeBack: "Que bom ver você de novo na Fernway.",
      email: "E-mail",
      password: "Senha",
      continue: "Continuar",
      twoStep: "Verificação em duas etapas",
      enterCode: "Digite o código de 6 dígitos do seu app autenticador.",
      verificationCode: "Código de verificação",
      verify: "Verificar",
      createTitle: "Crie sua conta",
      createLede: "Leva menos de um minuto.",
      newPassword: "Nova senha",
      createButton: "Criar conta",
      welcomeSam: "Olá, Sam",
      signedIn: "Você entrou na Fernway.",
      dashboard: "Ir para o painel",
    },
  },

  kit: {
    aria: "Ilustração de uma Folha de Recuperação do HavenKeys",
    caption: "Ilustração. A chave mostrada é inventada.",
  },

  developers: {
    heroTitle: "Como o HavenKeys funciona, de ponta a ponta.",
    heroLede:
      "O HavenKeys é pequeno de propósito. A criptografia vem de bibliotecas Rust consolidadas, as regras estão por escrito e os ataques que ele diz impedir são testes no código. Aqui está o projeto inteiro em uma página.",

    journeyTitle: "Acompanhe uma senha até em casa.",
    journeyLede:
      "O HavenKeys é um gerenciador de senhas com preenchimento automático cuidadoso, códigos de uso único e um cofre que funciona offline. O que o diferencia é o que acontece com uma senha entre o momento em que você a digita e o momento em que ela é preenchida. Este é o caminho, passo a passo.",

    browserTitle: "Preenchimento que espera o seu clique.",
    browserLede:
      "A extensão para Chrome e Firefox lê o formulário como você lê, oferece o que está salvo para aquele site e não faz nada até você escolher. Estes são os menus de verdade.",
    extensionListTitle: "Também na extensão",
    extensionFeatures: [
      {
        term: "Passkeys",
        text: "Entre com uma passkey salva pelo menu do campo, e salve novas. Depois que você entra com uma senha em um site que aceita passkeys, o HavenKeys pode criar uma passkey para você (dá para desligar isso).",
      },
      {
        term: "Entrada automática",
        text: "Depois que você escolhe um login, o HavenKeys pode clicar no botão de entrar e preencher a etapa seguinte e o código de uso único, no mesmo site, em até dois minutos. Nada acontece sem a sua escolha. Dá para desligar por login ou para o cofre inteiro.",
      },
      { term: "Editar no HavenKeys", text: "Pelo popup, abra um login no app de desktop para alterá-lo." },
    ],

    desktopTitle: "Um cofre que mora na sua mesa.",
    desktopLede:
      "O app de desktop guarda as chaves. Ele fica na bandeja do sistema, se tranca quando você se afasta e faz toda a criptografia no seu núcleo em Rust. A interface nunca vê uma chave, e uma senha só aparece na tela quando você a revela.",
    features: [
      { term: "Logins", text: "Usuários, senhas, sites com regras de correspondência, códigos de uso único e notas." },
      { term: "Notas seguras", text: "Códigos de recuperação, frases-senha, tudo que não é login. Cifradas por inteiro." },
      {
        term: "Passkeys",
        text: "Crie passkeys e entre com elas. A chave privada é criada e usada só no núcleo em Rust; a identidade do site também é conferida lá.",
      },
      {
        term: "Códigos de uso único",
        text: "SHA-1, SHA-256 ou SHA-512, seis ou oito dígitos. Cole um link otpauth:// uma única vez.",
      },
      {
        term: "Ler um QR code",
        text: "Configure um código de uso único lendo o QR code da área de transferência ou da tela. O segredo fica no Rust até você salvar.",
      },
      {
        term: "Gerador de senhas",
        text: "Você escolhe o tamanho e os tipos de caractere. Aleatoriedade do sistema operacional, sem viés.",
      },
      { term: "Busca", text: "Títulos, usuários e sites, buscados na memória. Nenhum índice em texto puro no disco." },
      {
        term: "Histórico de senhas",
        text: "As cinco últimas senhas de cada login, inclusive as trocadas pelo navegador.",
      },
      { term: "Importar e exportar", text: "Traga senhas do 1Password, Bitwarden, LastPass, KeePassXC, Chrome ou Firefox. Exporte um arquivo do Bitwarden, um CSV ou um backup criptografado do HavenKeys." },
      {
        term: "Bloqueio automático",
        text: "Após 5 a 60 minutos parado, ao suspender e ao sair. No Windows e no Linux, também quando a sessão é bloqueada.",
      },
      {
        term: "Secret Key no chaveiro do sistema",
        text: "Guardada no chaveiro do seu sistema (Windows Credential Manager, macOS Keychain, Secret Service no Linux).",
      },
      {
        term: "Abre ao ligar o computador",
        text: "Opcionalmente inicia junto com o computador, bloqueado, na bandeja do sistema, para que a extensão consiga alcançá-lo.",
      },
      {
        term: "Se atualiza sozinho",
        text: "Verifica se há atualizações assinadas e instala só quando você clica em Atualizar. Dá para desligar a verificação.",
      },
      {
        term: "Português e inglês",
        text: "O app segue o idioma do sistema, ou o que você escolher nas Configurações. A extensão segue o idioma do navegador.",
      },
    ],

    paperTitle: "Dois segredos. Um deles mora no papel.",
    paperP1:
      "A senha mestra é a que você lembra. A Secret Key são 128 bits aleatórios criados no seu dispositivo durante a configuração. Ela fica guardada em cada um dos seus computadores e impressa na sua Folha de Recuperação, e nunca vai para o servidor.",
    paperP2:
      "Assim, uma cópia roubada do banco de dados do servidor não vira um exercício de adivinhar senhas. Sem a Secret Key, o atacante precisa adivinhar as duas.",
    paperWarn:
      "Não existe recuperação de conta. Se você perder a Folha de Recuperação e todos os dispositivos que guardam a chave, o cofre se perde. Esse é o preço de ninguém mais conseguir abri-lo.",

    ledgerTitle: "Do que ele protege. Do que não protege.",
    ledgerLede:
      "Software de segurança conquista confiança sendo específico. Esta é a versão curta do modelo de ameaças, com as limitações incluídas.",
    defendsTitle: "Feito para impedir",
    defends: [
      "Alguém com uma cópia do banco de dados do servidor ou de um backup. Sem a sua Secret Key, ainda teria que adivinhar 128 bits aleatórios.",
      "Quem opera o servidor lendo o seu cofre. Ele guarda dados cifrados e não tem a chave deles.",
      "Páginas que falsificam formulários, escondem campos, embutem outros sites ou simulam cliques para disparar um preenchimento.",
      "Domínios parecidos. A correspondência usa a Public Suffix List, então fernway.example.evil.com é outro site.",
      "Segredos vazando em logs, mensagens de erro, URLs, notificações ou títulos de janela.",
    ],
    doesntTitle: "Não protege",
    doesnt: [
      "Malware rodando com o seu usuário enquanto o cofre está desbloqueado. Nenhum gerenciador de senhas local consegue impedir isso.",
      "Um servidor que apaga os seus dados. Ele é o único que escreve, então backups testados fazem parte de mantê-lo.",
      "Uma senha mestra fraca em um dispositivo copiado por inteiro, com a Secret Key junto.",
      "Ele não passou por auditoria independente, e os instaladores ainda não são assinados.",
    ],

    chainTitle: "Uma cadeia de chaves, sem atalhos.",
    chainLede:
      "A senha mestra nunca é usada para cifrar nada diretamente. Ela alimenta uma função que exige muita memória, é combinada com uma Secret Key aleatória e abre uma chave do cofre que foi aleatória desde o início. Trocar a senha mestra só recifra essa chave. Nenhum dos seus itens muda.",
    chainLink: "Projeto criptográfico completo",
    hierarchy: [
      { name: "Senha mestra", detail: "Nunca é guardada. Só serve de entrada para o Argon2id.", tone: "input" },
      { name: "Argon2id", detail: "128 MiB, 4 passadas, 4 faixas, um salt aleatório de 16 bytes.", tone: "op" },
      { name: "Chave mestra", detail: "32 bytes, só na memória.", tone: "key" },
      {
        name: "HKDF-SHA-256",
        detail: "Mistura a sua Secret Key de 128 bits, vinculada à sua conta e ao seu e-mail.",
        tone: "op",
      },
      {
        name: "Chave de cifragem de chaves",
        detail: "Abre a chave do cofre. Uma chave irmã faz o login no servidor e não abre nada.",
        tone: "key",
      },
      { name: "Chave do cofre", detail: "256 bits aleatórios do sistema. Guardada só cifrada.", tone: "key" },
      { name: "Chave de dados", detail: "Derivada por cofre. Fica na memória enquanto ele está aberto.", tone: "key" },
      {
        name: "Seus itens",
        detail: "AES-256-GCM, nonce novo a cada gravação, vinculado ao cofre, ao item e à função.",
        tone: "out",
      },
    ],

    zonesTitle: "Cinco lugares, cinco níveis de confiança.",
    zonesLede:
      "Cada parte do HavenKeys recebe só o que o seu trabalho exige. A fronteira que mais importa separa o núcleo em Rust de todo o resto: ele decide, e nada mais pode decidir por ele.",
    zoneGets: "Recebe",
    zoneLimits: "Limites",
    zones: [
      {
        name: "Núcleo em Rust",
        where: "Dentro do app de desktop",
        holds: "Chaves, decifragem e a verificação de origem de cada preenchimento",
        limit: "Confiável. É a parte em que você está confiando.",
      },
      {
        name: "Interface do desktop",
        where: "A janela do app",
        holds: "Um campo revelado por vez",
        limit: "Sem chaves, sem criptografia, sem acesso a arquivos ou rede, com CSP rígida",
      },
      {
        name: "Extensão do navegador",
        where: "Chrome ou Firefox",
        holds: "Títulos e usuários deste site; uma senha quando você escolhe um login",
        limit: "Cada pedido é conferido de novo em Rust. Ela nunca vê a chave do cofre.",
      },
      {
        name: "Páginas da web",
        where: "Em todo lugar que você navega",
        holds: "Nada, até você escolher um login para aquela página",
        limit: "Tratadas como hostis. Não conseguem nem mandar mensagem para a extensão.",
      },
      {
        name: "Seu servidor",
        where: "Um servidor seu ou nosso",
        holds: "Dados cifrados, além do seu e-mail e da quantidade, tamanho e data dos itens",
        limit: "Não tem chave para nada disso. Pode apagar dados, então mantenha backups.",
      },
    ],

    permTitle: "Uma extensão que pede menos.",
    permLede:
      "Oferecer salvar logins e usar chaves de acesso exige que a extensão rode nos sites que você visita, então ela pede isso na instalação. Ela não pede mais nada, não lê páginas com as quais você não interage, e o app de desktop decide quais logins cada site pode usar. Você pode retirar o acesso aos sites no navegador a qualquer momento.",
    notRequested: "Não solicitadas:",
    permHead: ["Permissão", "Por quê"],
    always: "Sempre",
    permissions: [
      { name: "nativeMessaging", optional: false, why: "O único caminho da extensão até o app de desktop." },
      {
        name: "activeTab",
        optional: false,
        why: "Ler o endereço da aba em que você clicou no botão da barra de ferramentas e preenchê-la. Só essa aba.",
      },
      {
        name: "scripting",
        optional: false,
        why: "Colocar o script de preenchimento nessa aba e registrá-lo nos sites a que a extensão tem acesso.",
      },
      {
        name: "https://*/*, http://*/*",
        optional: false,
        why: "Ofertas para salvar logins, chaves de acesso e sugestões na página. Você pode retirá-la nas configurações de extensões do navegador.",
      },
      {
        name: "storage",
        optional: false,
        why: "Uma configuração: se os logins aparecem abaixo dos campos de login. Mais nada.",
      },
    ],
    optionalTag: "Opcional, desligada por padrão",

    attacksTitle: "Doze ataques, escritos como testes.",
    attacksLede:
      "Cada um é um teste de regressão na suíte de testes do Rust ou da extensão, então uma mudança que o reabra faz os testes falharem.",
    attacksHead: ["Tentativa", "Resultado"],
    attacks: [
      ["Uma página em evil.com pede o login de github.com", "Negado"],
      ["A extensão pede um item pelo ID no site errado", "Negado, e parece igual a “não salvo aqui”"],
      ["Uma senha é pedida com o cofre trancado", "Negado"],
      ["O texto cifrado é modificado", "Falha na autenticação, nenhum texto puro"],
      ["Chega uma mensagem nativa malformada", "Rejeitada, sem travar"],
      ["Chega uma mensagem nativa grande demais", "Rejeitada pelo limite de tamanho"],
      ["Uma página cria milhares de campos", "Sem lentidão significativa"],
      ["Um bloco cifrado é trocado entre itens ou funções", "Falha na autenticação"],
      ["Um cofre com versão de formato desconhecida", "Recusado com segurança"],
      ["Um quadro de login de github.com embutido em evil.com", "Negado: a página principal também precisa bater"],
      ["Uma página simula cliques ou teclas para disparar um preenchimento", "Ignorado"],
      ["O quadro navega para outro lugar antes do preenchimento chegar", "Recusado"],
    ],

    scopeTitle: "Do que ele não vai proteger você.",
    scopeLede:
      "Nenhum gerenciador de senhas local consegue prometer tudo. Estes são os limites, ditos logo de cara em vez de descobertos depois.",
    outOfScope: [
      "Malware rodando com o seu usuário enquanto o cofre está desbloqueado: ele pode ler a memória, registrar teclas ou pedir logins ao app de desktop como a extensão faz.",
      "Comprometimento do kernel ou do root, ataques de hardware, ataques de cold boot e DMA.",
      "Perícia de memória depois do bloqueio. As chaves são zeradas onde o código tem controle, mas cópias podem sobrar em lugares que ele não controla.",
      "Reversão do arquivo do cofre, e um servidor que reenvia uma senha antiga de um item.",
      "Uma senha mestra fraca, e gerenciadores de área de transferência lendo uma senha copiada antes de ela ser apagada.",
    ],

    readingTitle: "Leia os documentos originais.",
    readingLede: "Tudo acima é um resumo. Estes são os documentos que ele resume, em inglês.",
    reading: [
      { title: "Modelo de ameaças", file: "threat-model.md", body: "Do que o HavenKeys protege, e do que explicitamente não protege." },
      { title: "Modelo de segurança", file: "security-model.md", body: "Como cada defesa é aplicada, permissão por permissão." },
      { title: "Criptografia", file: "crypto.md", body: "A hierarquia de chaves, o formato dos blocos e os parâmetros exatos." },
      {
        title: "Revisão de segurança",
        file: "security-review.md",
        body: "O que apareceu ao revisar este código contra o próprio modelo de ameaças, incluindo o que ainda está aberto.",
      },
      { title: "Auto-hospedagem", file: "self-hosting.md", body: "Rode seu próprio servidor com Docker ou Railway." },
      { title: "Arquitetura", file: "architecture.md", body: "Como o app de desktop, a extensão e o núcleo em Rust se encaixam." },
      { title: "Mensagens nativas", file: "native-messaging.md", body: "O protocolo entre a extensão e o app de desktop, e como ele é validado." },
      { title: "Referência de implantação", file: "deployment.md", body: "Variáveis, Railway, backups." },
    ],

    buildTitle: "Compile você mesmo",
    buildBody: (
      <>
        O HavenKeys é código aberto, sob Apache-2.0. O{" "}
        <Ext href={`${DOCS}development.md`}>guia de desenvolvimento</Ext> explica como compilar o app
        de desktop, a extensão, o Android e o servidor; o{" "}
        <Ext href={`${DOCS}self-hosting.md`}>self-hosting.md</Ext> explica como rodar seu próprio servidor.
      </>
    ),
  },

  security: {
    heroTitle: "Feito para que só você abra o seu cofre.",
    heroLede: "O que o HavenKeys protege, o que não consegue proteger, e onde conferir por conta própria.",
    protectsTitle: "O que ele protege",
    protects: [
      { title: "Seu cofre, onde quer que esteja", text: "Senhas, notas, códigos e passkeys são trancados no seu dispositivo antes de serem salvos ou enviados. O servidor guarda uma cópia que ele não tem como abrir." },
      { title: "Um servidor roubado", text: "Quem copiar os dados do servidor ainda precisa da sua senha mestra e da sua Secret Key, que nunca chegam ao servidor de um jeito que alguém consiga ler (a Secret Key também fica na sua Folha de Recuperação)." },
      { title: "Sites falsos", text: "A extensão só oferece um login no site para o qual ele foi salvo, e só preenche depois do seu clique. Endereços parecidos não valem." },
      { title: "Vazamentos acidentais", text: "Senhas nunca aparecem em logs, notificações ou títulos de janela, e senhas copiadas são apagadas da área de transferência." },
    ],
    limitsTitle: "O que ele não consegue fazer",
    limits: [
      "Proteger você de um malware já rodando no seu computador enquanto o cofre está aberto.",
      "Recuperar um cofre se você perder a Folha de Recuperação e todos os dispositivos. Não existe recuperação de conta: é o preço de ninguém mais ter uma chave.",
      "Restaurar um servidor seu que foi perdido sem backup.",
    ],
    auditTitle: "Honesto sobre onde está",
    auditBody: "O HavenKeys é código aberto e seu projeto de segurança está documentado por inteiro, mas ainda não passou por uma auditoria independente.",
    deepTitle: "Quer os detalhes?",
    deepBody: "Como suas chaves são criadas, o que a extensão pode acessar, os ataques contra os quais ele é testado e o modelo de ameaças completo estão na página Desenvolvedores.",
    deepCta: "Ler os detalhes técnicos",
  },

  download: {
    title: "Baixar o HavenKeys",
    latest: "Versão mais recente",
    checking: "Buscando a versão mais recente…",
    unavailable: "Não foi possível carregar a versão mais recente. Talvez nenhuma tenha sido publicada ainda.",
    seeReleases: "Ver as versões no GitHub",
    installersAria: "Instaladores",
    yourSystem: "Seu sistema",
    download: "Baixar",
    viewReleases: "Ver versões",
    android: {
      label: "Android",
      format: "APK para Android 9 ou mais recente",
      early: "Versão inicial",
      downloadApk: "Baixar APK",
      checksum: "Checksum SHA-256",
      certificate: "Certificado de assinatura (SHA-256)",
      comingSoon: "O app para Android ainda não foi lançado.",
      stepsTitle: "Instalando no Android",
      steps: [
        { title: "Instale o APK", body: "Baixe e abra. O Android pede uma vez para permitir instalar apps pelo navegador." },
        { title: "Entre", body: "Abra o HavenKeys e escaneie a sua Folha de Recuperação, ou use seu código de configuração." },
        { title: "Ative o preenchimento automático", body: "No HavenKeys, abra Configurações → Configurar preenchimento automático." },
        { title: "Use no Chrome", body: "Abra Configurações → Serviços de preenchimento automático e escolha “Preenchimento automático com outro serviço”." },
      ],
      browsers:
        "O preenchimento funciona em apps, no Chrome e no Firefox. O Samsung Internet só deixa preencher os gerenciadores de senhas da lista da própria Samsung, e o HavenKeys não está nela.",
    },
    platforms: {
      windows: { label: "Windows", format: "instalador .msi" },
      macos: { label: "macOS", format: "imagem de disco .dmg" },
      "linux-appimage": { label: "Linux", format: ".AppImage, roda em qualquer distribuição" },
      "linux-deb": { label: "Debian e Ubuntu", format: "pacote .deb" },
    },
    beforeTitle: "Antes de instalar",
    steps: [
      {
        title: "Tenha sua conta",
        body: (
          <>
            O HavenKeys guarda seu cofre trancado num servidor. Crie uma conta no nosso, ou{" "}
            <Link to="/pt-br/self-host">rode seu próprio servidor</Link>. Depois imprima sua Folha de Recuperação:
            é com ele que você entra num dispositivo novo.
          </>
        ),
        actions: (
          <Link className="btn btn--ghost btn--sm" to="/pt-br/signup">
            Criar conta
          </Link>
        ),
      },
      {
        title: "Adicione a extensão do navegador",
        body: (
          <>
            Instale pela <Ext href={CHROME_STORE}>Chrome Web Store</Ext> ou pelo{" "}
            <Ext href={FIREFOX_STORE}>Firefox Add-ons</Ext>. Os instaladores já incluem a parte que
            conecta o navegador ao app, e a registram no Chrome e no Firefox. Depois, desbloqueie o
            HavenKeys e ative <strong>Configurações → Extensão do navegador</strong>.
          </>
        ),
        actions: (
          <>
            <a className="btn btn--ghost btn--sm" href={CHROME_STORE} target="_blank" rel="noopener noreferrer">
              <Icon name="external" size={15} />
              Adicionar ao Chrome
            </a>
            <a className="btn btn--ghost btn--sm" href={FIREFOX_STORE} target="_blank" rel="noopener noreferrer">
              <Icon name="external" size={15} />
              Adicionar ao Firefox
            </a>
          </>
        ),
      },
      {
        title: "Ele se atualiza sozinho",
        body: "A partir da 0.9.0, o HavenKeys verifica se há atualizações assinadas e instala uma quando você clica em Atualizar. Instalações .deb e .rpm recebem um botão Baixar que abre a página de versões. Na 0.8.0 ou anterior, instale a 0.9.0 uma vez, manualmente, por cima da instalação existente — sem precisar desinstalar antes.",
      },
      {
        title: "Espere um aviso na primeira execução",
        body: "Os instaladores ainda não são assinados, então o Windows SmartScreen e o macOS Gatekeeper vão mostrar um aviso. As versões para Windows e macOS são mais novas e menos testadas que a de Linux.",
      },
    ],
  },

  privacy: {
    title: "Política de Privacidade",
    updated: "Atualizada em 9 de outubro de 2026.",
    body: (
      <>
        <h2>Este site</h2>
        <p>
          O havenkeys.net não usa cookies. Ele é hospedado na Vercel, que processa registros padrão
          de acesso (como endereço IP e user agent do navegador) para servir o site, e usa o Vercel
          Analytics, que conta visualizações de página de forma agregada, sem cookies nem
          rastreamento entre sites. A página de download consulta a API pública do GitHub para
          saber qual é a versão mais recente diretamente do seu navegador, então o GitHub também vê
          essa requisição. Se você escolher um idioma no seletor, o site guarda essa escolha no
          armazenamento local do seu navegador; ela nunca é enviada a lugar nenhum. Não há
          publicidade nem nenhum outro script de terceiros no site.
        </p>
        <p>
          A página Criar conta envia seu endereço de e-mail, seu idioma e a versão dos Termos que
          você aceitou ao nosso servidor em api.havenkeys.net, que envia a você um código e, depois
          que você o confirma, um código de configuração. O código de configuração aparece na página
          e fica apenas na memória dela; não é guardado no seu navegador nem no nosso analytics. O
          nosso servidor registra seu endereço de e-mail, o idioma que você escolheu (para escrever
          a você nele) e a versão dos Termos que você aceitou, e envia avisos da conta (o código, o
          código de configuração e quando um período de teste está para acabar ou acabou) por meio
          de um provedor de e-mail que atua como operador por nós; seu endereço não é usado para
          mais nada além dos avisos da conta. O código de cadastro é guardado apenas como um hash
          com chave, por 15 minutos. Para limitar abusos, as tentativas de cadastro são contadas por
          endereço de rede e por endereço de e-mail durante uma hora. Nunca enviamos e-mail de
          marketing.
        </p>

        <h2>O app de desktop e a extensão do navegador</h2>
        <p>
          O aplicativo HavenKeys não tem telemetria, analytics nem relatórios de falha. A sua senha
          mestra e a sua Secret Key nunca são enviadas ao servidor nem à extensão do navegador; para
          fazer login, o app envia ao servidor uma chave derivada delas. A sua Secret Key só sai do
          seu dispositivo do jeito que você escolher: na sua Folha de Recuperação e quando você a digita em
          outro dispositivo seu. As chaves do cofre são geradas no seu dispositivo e nunca saem dele
          sem estarem cifradas.
        </p>
        <p>
          Todo cofre pertence a uma conta em um <code>havenkeys-server</code> operado por você, pelo
          servidor em que você criou sua conta, ou o seu próprio. Esse servidor guarda o seu cofre cifrado, que ele não consegue
          decifrar, além dos metadados de que precisa para servi-lo: o ID do cofre, o e-mail da sua
          conta, os parâmetros e o salt da derivação de chaves, a chave do cofre cifrada, as revisões
          dos itens e a quantidade e o tamanho aproximado dos seus itens. Ele também guarda o status do plano da sua conta (teste, ativa, congelada) e quando ele mudou. Para limitar tentativas de
          login, ele também conta as tentativas que falharam por conta e por endereço de rede, e
          zera essa contagem depois de um login bem-sucedido. O{" "}
          <Ext href={`${DOCS}server-sync.md`}>projeto de sincronização com o servidor</Ext> (em
          inglês) descreve isso em detalhes. Os desenvolvedores também operam um servidor para outras pessoas, mantido por SAMUEL
          DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA, que é a controladora dos dados pessoais
          listados aqui para as contas nele. Esse servidor também não consegue decifrar cofres.
        </p>

        <h2>A extensão do navegador</h2>
        <p>
          A extensão HavenKeys tem um único propósito: preencher, salvar e gerar logins do app de
          desktop HavenKeys nos sites que você usa. Para isso ela lida com os dados abaixo, e só
          quando você a usa:
        </p>
        <ul>
          <li>
            <strong>O endereço da página</strong> (e do frame que contém o formulário de login) em
            que você abre a extensão ou as sugestões dela, para que o app de desktop encontre os
            logins salvos para aquele site.
          </li>
          <li>
            <strong>Os campos de login dessa página</strong>: tipos, nomes, rótulos e o texto ao
            redor, para reconhecer os campos de usuário, senha e código de uso único. Isso é lido
            dentro da página e nunca é enviado a lugar nenhum, nem mesmo ao app de desktop.
          </li>
          <li>
            <strong>Usuários, senhas e códigos de uso único</strong> que você escolhe preencher, que
            o app de desktop envia só para aquele site, e senhas que a extensão gera para você.
          </li>
          <li>
            <strong>Um usuário e uma senha que você envia em um formulário de login</strong>, para
            perguntar se você quer salvá-los. Nada é salvo sem a sua confirmação.
          </li>
        </ul>
        <p>
          Os endereços das páginas e os logins que você salva vão apenas para o app de desktop
          HavenKeys no mesmo computador, pelo canal de native messaging do navegador. Eles nunca são
          enviados para nós nem para terceiros. Quando você confirma que quer salvar, o app de
          desktop cifra o login no seu dispositivo e o envia, cifrado, ao seu próprio{" "}
          <code>havenkeys-server</code> por HTTPS (HTTP simples só é aceito para um servidor no
          mesmo computador). O servidor não consegue decifrá-lo.
        </p>
        <p>
          A extensão não guarda logins, conteúdo de páginas nem endereços no armazenamento do
          navegador, em cookies ou no disco. O único valor que ela guarda é uma configuração: se os
          logins aparecem abaixo dos campos de login. Um login que ela preenche fica na memória só
          enquanto ela preenche a página. Um login aguardando para ser salvo fica na memória por no
          máximo três minutos e é descartado assim que você o salva, dispensa o aviso ou o cofre é
          bloqueado. A extensão roda nos sites que você visita para poder oferecer salvar logins e
          cuidar de chaves de acesso; você pode retirar esse acesso nas configurações de extensões
          do navegador.
        </p>

        <h2>O que nunca coletamos</h2>
        <p>
          Nós, os desenvolvedores do HavenKeys, não recebemos nenhum dado seu. Em particular, nunca
          recebemos:
        </p>
        <ul>
          <li>Senhas mestras ou Secret Keys</li>
          <li>Chaves de cifragem do cofre</li>
          <li>Senhas, usuários, segredos TOTP ou notas seguras guardados</li>
          <li>
            O seu histórico de navegação ou o conteúdo das páginas que você visita. A extensão lida
            com endereços de páginas e formulários de login apenas no seu dispositivo, como descrito
            acima.
          </li>
        </ul>

        <h2>Como os dados são usados e compartilhados</h2>
        <p>
          Os dados são usados apenas para oferecer os recursos descritos acima. Eles nunca são
          vendidos, nunca são usados nem compartilhados para publicidade e nunca são usados para
          avaliar crédito ou conceder empréstimos. Ninguém lê os seus dados: não temos acesso a eles,
          e o servidor guarda o seu cofre apenas de forma cifrada.
        </p>
        <p>
          O uso das informações recebidas pela extensão HavenKeys segue a{" "}
          <Ext href="https://developer.chrome.com/docs/webstore/program-policies/">
            Política de Dados do Usuário da Chrome Web Store
          </Ext>{" "}
          (em inglês), incluindo os requisitos de Uso Limitado (Limited Use).
        </p>

        <h2>Guarda e exclusão dos seus dados</h2>
        <p>
          O seu cofre é guardado até você excluí-lo. Excluir um item o remove de todos os
          dispositivos. No servidor, o conteúdo cifrado dele é apagado e só fica um marcador (o ID
          aleatório do item e o horário da exclusão), para que os seus outros dispositivos saibam que
          devem removê-lo também. "Remove this device" (remover este dispositivo), no app de
          desktop, desconecta o computador e deixa a cópia local do cofre de lado; depois disso você
          pode excluir esse arquivo. Você pode excluir a sua conta pelo próprio app;{" "}
          <a href="/pt-br/delete-account">Excluir a sua conta</a> diz o que é apagado e quando. Desinstalar a extensão a remove por completo, já que ela não guarda dados
          próprios. Contas nunca ativadas em até 7 dias após o cadastro são apagadas automaticamente.
        </p>

        <h2>Contato</h2>
        <p>
          Dúvidas sobre esta política podem ser enviadas como uma issue no{" "}
          <Ext href={`${GH}/issues`}>GitHub</Ext>.{" "}
          Para pedidos de privacidade, incluindo exclusão:{" "}
          <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a>.
        </p>
      </>
    ),
  },

  deleteAccount: {
    title: "Excluir a sua conta",
    updated: "Atualizada em 5 de outubro de 2026.",
    body: (
      <>
        <h2>Pelo app</h2>
        <p>
          No desktop: Configurações → <strong>Excluir conta e todos os dados</strong>. No Android:
          Configurações → Conta → <strong>Excluir conta e todos os dados</strong>. Você confirma com
          o e-mail da conta e a sua senha mestra. Faça um backup cifrado antes se puder querer seus
          dados depois: a exclusão não pode ser desfeita.
        </p>

        <h2>O que é apagado, e quando</h2>
        <ul>
          <li>
            <strong>Imediatamente:</strong> o seu cofre cifrado e os itens, seus dispositivos e
            sessões, seu endereço de e-mail, os parâmetros de derivação de chave e o contador de tentativas de login da sua conta. A cópia no dispositivo que você usou também é apagada, e os outros
            dispositivos apagam as deles na próxima vez que se conectarem, se for em até 30 dias.
          </li>
          <li>
            <strong>Em até 30 dias:</strong> impressões digitais anônimas das suas sessões antigas,
            guardadas só para que os outros dispositivos saibam que a conta não existe mais. Elas
            não contêm e-mail, nome nem identificador da conta.
          </li>
          <li>
            <strong>Em até 30 dias:</strong> cópias em backups do banco de dados e em logs do
            servidor, que expiram sozinhas.
          </li>
          <li>
            <strong>Não ligado a você:</strong> contagens de tentativas de login que falharam,
            guardadas por endereço de rede e que nunca identificam uma conta, não fazem parte da
            exclusão.
          </li>
        </ul>
        <p>
          Backups que você mesmo exportou, e cópias separadas num dispositivo por um "Remover este
          dispositivo" anterior, são seus e não são tocados.
        </p>

        <h2>Perdeu o acesso à conta?</h2>
        <p>
          Envie um e-mail para{" "}
          <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a> a partir do
          endereço da conta. Confirmaremos o pedido e excluiremos a conta do mesmo jeito que o app
          faz.
        </p>
      </>
    ),
  },

  terms: {
    title: "Termos de Serviço",
    updated: "Atualizados em 9 de outubro de 2026.",
    body: (
      <>
        <h2>Licença</h2>
        <p>
          O HavenKeys é software de código aberto, sob a{" "}
          <Ext href={`${GH}/blob/main/LICENSE-APACHE`}>Licença Apache 2.0</Ext>. Você pode usá-lo,
          modificá-lo e redistribuí-lo nos termos dela.
        </p>
        <p>
          Este site, o app de desktop e a extensão do navegador incorporam as fontes Hanken Grotesk,
          Source Serif 4 e JetBrains Mono, licenciadas à parte sob a SIL Open Font License 1.1. Os avisos de copyright e essa licença estão em{" "}
          <Ext href={`${GH}/blob/main/THIRD-PARTY-NOTICES.md`}>THIRD-PARTY-NOTICES.md</Ext>.
        </p>

        <h2>Sem garantia</h2>
        <p>
          O HavenKeys é fornecido "no estado em que se encontra", sem garantia de nenhum tipo,
          expressa ou implícita, incluindo, mas não se limitando a, adequação a uma finalidade
          específica. Este software não passou por uma auditoria de segurança independente e não
          deve ser considerado um substituto para gerenciadores de senhas auditados
          profissionalmente em uso de produção de alto valor. O uso é por sua conta e risco.
        </p>

        <h2>Hospedagem própria</h2>
        <p>
          Se você roda o <code>havenkeys-server</code>, é o único responsável pela implantação,
          pela disponibilidade e pelos backups dele. O servidor é a cópia oficial do seu cofre;
          perdê-lo sem um backup testado significa perder os seus dados. Leia o{" "}
          <Ext href={`${DOCS}self-hosting.md`}>guia de hospedagem própria</Ext> e o{" "}
          <Ext href={`${DOCS}deployment.md`}>guia de implantação</Ext> (em inglês), principalmente a
          seção sobre backups, antes de guardar qualquer coisa que você não pode perder.
        </p>

        <h2>Nosso servidor e o seu</h2>
        <p>
          SAMUEL DA SILVA ROCHA DESENVOLVIMENTO DE SOFTWARE LTDA (“nós”) mantém um{" "}
          <code>havenkeys-server</code> em api.havenkeys.net. Qualquer pessoa pode, em vez disso,
          rodar o próprio servidor; baixar o software não cria conta conosco.
        </p>

        <h2>Contas, período de teste e planos</h2>
        <p>
          Criar uma conta no nosso servidor inicia um período de teste gratuito de 14 dias, sem
          cartão. Depois dele, a conta continua no plano Pessoal quando você assina; até as
          assinaturas abrirem, mantemos contas abertas a pedido. Contas que convidamos diretamente
          são cortesia sem data de término, salvo aviso.
        </p>
        <p>
          Uma conta cujo período de teste terminou, ou cujo pagamento falhou, fica{" "}
          <strong>congelada</strong>: o servidor recusa alterações, dispositivos novos e passkeys
          novas, e os apps param de preencher. Você continua lendo seu cofre em todos os
          dispositivos, entrando com passkeys já salvas e exportando, aberto ou criptografado, a
          qualquer momento. Nunca apagamos um cofre por estar congelado; você pode apagar a conta
          pelo app quando quiser.
        </p>
        <p>
          O serviço é prestado com o melhor esforço, sem acordo de nível de serviço, e pode mudar
          ou terminar com aviso. Nada aqui limita seu direito de exportar e sair.
        </p>
      </>
    ),
  },

  pricing: {
    title: "Um plano, suas chaves",
    lede: "O HavenKeys é código aberto e gratuito para hospedar por conta própria. O serviço hospedado paga o servidor e o trabalho.",
    plan: "Pessoal",
    priceSoon: "Preço em breve",
    trial: "14 dias grátis, sem cartão",
    includes: [
      "Sem limite de logins, cartões, identidades, notas e passkeys",
      "Desktop, extensão do navegador e Android",
      "Entrada em um computador novo aprovada pelo celular",
      "Exportação a qualquer momento, aberta ou criptografada",
    ],
    afterTitle: "Depois do período de teste",
    keepsTitle: "Continua funcionando",
    keeps: [
      "Seu cofre continua legível em todos os dispositivos",
      "Exportação, aberta ou criptografada",
      "Entrar com uma passkey que você já salvou",
    ],
    pausesTitle: "Fica pausado até você assinar",
    pauses: [
      "Alterações no cofre",
      "Dispositivos novos",
      "Preenchimento automático",
      "Salvar passkeys novas",
    ],
    subscribeSoon: (
      <>
        As assinaturas abrem em breve. Até lá, escreva para{" "}
        <a href="mailto:samuelsilv.rocha@gmail.com">samuelsilv.rocha@gmail.com</a> e mantemos sua
        conta aberta.
      </>
    ),
    cta: "Criar conta",
    selfHost: "Ou rode seu próprio servidor de graça",
  },

  selfHost: {
    title: "Rode seu próprio servidor HavenKeys",
    lede: "Guarde seu cofre trancado num servidor que você controla. Você precisa de um domínio, um pequeno servidor e alguns minutos.",
    needTitle: "O que você precisa",
    needs: [
      "Um pequeno servidor Linux com Docker, Docker Compose v2 e curl — cerca de US$5 por mês, ou um computador em casa sempre ligado.",
      "Um domínio ou subdomínio que você possa apontar para ele, como cofre.exemplo.com.",
      "Ou, no lugar dos dois: uma conta no Railway.",
    ],
    stepsTitle: "Quatro passos com Docker",
    steps: [
      { title: "Aponte seu domínio para o servidor", text: "Crie um registro A do seu domínio com o IP do servidor, e libere as portas 80 e 443.", code: "" },
      { title: "Baixe o pacote", text: "Seis arquivos pequenos: os serviços, o HTTPS, os backups e o script de instalação.", code: "mkdir havenkeys && cd havenkeys\nfor f in compose.yaml Caddyfile .env.example setup.sh restore.sh backup.sh; do\n  curl -fsSLO \"https://raw.githubusercontent.com/rochasamuel/havenkeys/main/deploy/compose/$f\"\ndone\nchmod +x setup.sh restore.sh backup.sh" },
      { title: "Rode a instalação", text: "O script pede seu domínio e e-mail, cria os segredos e sobe tudo com HTTPS.", code: "./setup.sh" },
      { title: "Crie sua conta", text: "O comando mostra um convite uma única vez. Cole-o no app HavenKeys do seu computador.", code: "docker compose exec server havenkeys-server admin new-account \\\n  --email voce@exemplo.com --server-url https://cofre.exemplo.com" },
    ],
    railwayTitle: "Ou publique no Railway",
    railwayBody: "Sem servidor para cuidar: o Railway roda o HavenKeys e o banco de dados para você, por cerca de US$5 por mês.",
    railwayCta: "Publicar no Railway",
    railwaySoon: "O modelo de um clique para o Railway chega em breve. Até lá, o guia tem os passos para o Railway.",
    backupsTitle: "Backups inclusos",
    backupsBody: "O pacote salva uma cópia do banco de dados toda noite e guarda os últimos 14 dias numa pasta de backups. Copie essa pasta para outro lugar com frequência, e teste uma restauração uma vez para saber que funciona.",
    chargeTitle: "A responsabilidade é sua",
    chargeBody: "Quando você roda o servidor, mantê-lo no ar e com backup é com você. O HavenKeys não recupera um cofre de um servidor perdido sem backup.",
    guideCta: "Ler o guia completo",
    inviteInstead: "Prefere não rodar um servidor? Crie uma conta no nosso.",
  },

  signup: {
    title: "Crie sua conta HavenKeys",
    step: (n: number) => `Passo ${n} de 3`,
    lede: "Três passos: confirme seu e-mail e depois configure o app no computador ou no celular.",
    emailLabel: "E-mail",
    emailHint: "Enviamos um código de seis dígitos para confirmar que ele é seu.",
    terms: (terms: string, privacy: string) => (
      <>
        Li e aceito os <Link to={terms}>Termos</Link> e a <Link to={privacy}>Política de Privacidade</Link>.
      </>
    ),
    create: "Criar conta",
    sending: "Enviando…",
    codeTitle: "Veja seu e-mail",
    codeLede: (email: string) => (
      <>
        Enviamos um código de seis dígitos para <strong>{email}</strong>. Ele vale por 15 minutos. Se este
        endereço já tem uma conta, o e-mail diz isso.
      </>
    ),
    codeLabel: "Código",
    verify: "Continuar",
    verifying: "Verificando…",
    resend: "Reenviar código",
    resendIn: "Reenviar em {s} s",
    changeEmail: "Usar outro e-mail",
    doneTitle: "Seu código de configuração",
    doneLede:
      "Instale o HavenKeys, abra o app, escolha “Tenho um código de configuração” e cole este código. Depois escolha uma senha mestra e guarde bem sua Folha de Recuperação.",
    copy: "Copiar",
    copied: "Copiado",
    alsoEmailed: "Também enviamos por e-mail. Ele vale por 24 horas e funciona uma única vez.",
    inviteAria: "Código de configuração",
    downloadsTitle: "Baixe o app",
    otherDownloads: "Todos os downloads",
    keepTab: "Mantenha esta aba aberta até colar o código no app.",
    errors: {
      email: "Isso não parece um endereço de e-mail.",
      terms: "Aceite os Termos e a Política de Privacidade para continuar.",
      code: "Digite os seis dígitos do e-mail.",
      invalid: "Esse código não é válido. Confira o e-mail ou peça um novo código.",
      rate_limited: "Muitas tentativas. Espere uma hora e tente de novo.",
      unavailable: "Não conseguimos enviar o e-mail agora. Tente de novo em alguns minutos.",
      closed: "O cadastro abre em breve. Escreva para samuelsilv.rocha@gmail.com e cuidamos disso enquanto isso.",
      emailRejected: "Esse endereço de e-mail não foi aceito.",
      network: "Não conseguimos falar com o servidor. Verifique sua conexão e tente de novo.",
    },
  },

  notFound: {
    title: "Página não encontrada",
    body: (home: string) => (
      <>
        Não há nada neste endereço. <Link to={home}>Ir para a página inicial</Link>.
      </>
    ),
  },
};
