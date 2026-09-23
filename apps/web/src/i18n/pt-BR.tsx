import type { ReactNode } from "react";
import { Link } from "react-router-dom";
import type { Messages } from "./en";

/*
 * Português do Brasil. Same shape as en.tsx, enforced by the Messages type.
 * Secret Key, Emergency Kit and havenkeys-server keep their English names:
 * they are what the app itself shows.
 */

const GH = "https://github.com/rochasamuel/havenkeys";
const DOCS = `${GH}/blob/main/docs/`;

function Ext({ href, children }: { href: string; children: ReactNode }) {
  return (
    <a href={href} target="_blank" rel="noreferrer">
      {children}
    </a>
  );
}

export const ptBR: Messages = {
  meta: {
    title: "HavenKeys — um gerenciador de senhas que é realmente seu",
    description:
      "HavenKeys é um gerenciador de senhas local-first: um app de desktop com núcleo de segurança em Rust, uma extensão de navegador e um servidor que você mesmo roda.",
  },

  common: {
    disclaimer:
      "Este software não passou por uma auditoria de segurança independente e não deve ser considerado um substituto para gerenciadores de senhas auditados profissionalmente em uso de produção de alto valor.",
    downloadCta: "Baixar o HavenKeys",
  },

  nav: {
    homeAria: "Início do HavenKeys",
    mainAria: "Principal",
    howItWorks: "Como funciona",
    browser: "Navegador",
    security: "Segurança",
    githubAria: "HavenKeys no GitHub",
    download: "Baixar",
    switchShort: "EN",
    switchAria: "View the site in English",
  },

  footer: {
    tagline: "Um gerenciador de senhas que você mesmo hospeda.",
    aria: "Rodapé",
    download: "Baixar",
    security: "Segurança",
    github: "GitHub",
    privacy: "Privacidade",
    terms: "Termos e licença",
    license: "MIT ou Apache-2.0",
    languageAria: "Idioma",
  },

  home: {
    heroTitle: (
      <>
        Senhas que só saem de casa <em>trancadas</em>.
      </>
    ),
    heroLede:
      "Suas chaves ficam nos seus dispositivos; o único servidor é um que você roda, e ele guarda dados cifrados que não tem como abrir.",
    seeHow: "Veja como funciona",
    heroMeta: "Gratuito e open source · Windows, macOS e Linux · Chrome e Firefox",
    placeholderLabel: "Captura de tela do app de desktop",
    placeholderNote: "A tela do cofre entra aqui.",
    popupAlt: "O popup do HavenKeys no navegador: dois logins salvos da Fernway, um botão Fill e um código de uso único.",
    menuAlt: "O menu do HavenKeys na página oferecendo dois logins salvos.",

    journeyTitle: "Acompanhe uma senha até em casa.",
    journeyLede:
      "O HavenKeys é um gerenciador de senhas com preenchimento automático cuidadoso, códigos de uso único e um cofre que funciona offline. O que o diferencia é o que acontece com uma senha entre o momento em que você a digita e o momento em que ela é preenchida. Este é o caminho, passo a passo.",

    browserTitle: "Preenchimento que espera o seu clique.",
    browserLede:
      "A extensão para Chrome e Firefox lê o formulário como você lê, oferece o que está salvo para aquele site e não faz nada até você escolher. Estes são os menus de verdade.",

    desktopTitle: "Um cofre que mora na sua mesa.",
    desktopLede:
      "O app de desktop guarda as chaves. Ele fica na bandeja do sistema, se tranca quando você se afasta e faz toda a criptografia no seu núcleo em Rust. A interface nunca vê uma chave, e uma senha só aparece na tela quando você a revela.",
    features: [
      { term: "Logins", text: "Usuários, senhas, sites com regras de correspondência, códigos de uso único e notas." },
      { term: "Notas seguras", text: "Códigos de recuperação, frases-senha, tudo que não é login. Cifradas por inteiro." },
      {
        term: "Códigos de uso único",
        text: "SHA-1, SHA-256 ou SHA-512, seis ou oito dígitos. Cole um link otpauth:// uma única vez.",
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
      {
        term: "Importar do 1Password",
        text: "Traga uma exportação .1pux: logins, notas e códigos de uso único vêm junto.",
      },
      {
        term: "Bloqueio automático",
        text: "Após 5 a 60 minutos parado, ao suspender e ao sair. No Windows e no Linux, também quando a sessão é bloqueada.",
      },
    ],

    paperTitle: "Dois segredos. Um deles mora no papel.",
    paperP1:
      "A senha mestra é a que você lembra. A Secret Key são 128 bits aleatórios criados no seu dispositivo durante a configuração. Ela fica guardada em cada um dos seus computadores e impressa no seu Emergency Kit, e nunca vai para o servidor.",
    paperP2:
      "Assim, uma cópia roubada do banco de dados do servidor não vira um exercício de adivinhar senhas. Sem a Secret Key, o atacante precisa adivinhar as duas.",
    paperWarn:
      "Não existe recuperação de conta. Se você perder o kit e todos os dispositivos que guardam a chave, o cofre se perde. Esse é o preço de ninguém mais conseguir abri-lo.",

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
    securityOverview: "Ler a visão geral de segurança",

    closerTitle: (
      <>
        Leve suas senhas para <em>casa</em>.
      </>
    ),
    closerLede: "Instale o app de desktop, aponte para o seu servidor e adicione a extensão.",
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
            mesmo roda. Não há conta em fornecedor nem empresa no meio guardando o seu cofre.
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
      secretWhere: "Nos seus dispositivos e no seu Emergency Kit",
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
      where: "no hardware que você escolher",
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
        body: "As sugestões na página ficam desligadas até você ativá-las. Sem elas, o botão da barra de ferramentas preenche a aba atual, com as mesmas verificações de origem e nada rodando em páginas em que você não clicou.",
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
    aria: "Ilustração de um Emergency Kit do HavenKeys",
    caption: "Ilustração. A chave mostrada é inventada.",
  },

  security: {
    heroTitle: "Segurança que você pode ler de ponta a ponta.",
    heroLede:
      "O HavenKeys é pequeno de propósito. A criptografia vem de bibliotecas Rust consolidadas, as regras estão por escrito e os ataques que ele diz impedir são testes no código. Aqui está o projeto inteiro em uma página.",

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
        where: "No hardware que você escolher",
        holds: "Dados cifrados, além do seu e-mail e da quantidade, tamanho e data dos itens",
        limit: "Não tem chave para nada disso. Pode apagar dados, então mantenha backups.",
      },
    ],

    permTitle: "Uma extensão que pede menos.",
    permLede:
      "A maior parte do valor, preencher com verificação de origem, funciona só com a aba em que você clica. Acesso a todos os sites é o que uma página maliciosa ou uma versão comprometida mais gostaria de ter, então isso é você quem concede, não uma condição para instalar.",
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
        why: "Colocar o script de preenchimento nessa aba, ou registrá-lo quando as sugestões na página estão ligadas.",
      },
      {
        name: "https://*/*, http://*/*",
        optional: true,
        why: "Sugestões na página e avisos para salvar. Só é pedida quando você as ativa, e você pode limitá-la aos sites que escolher.",
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
    ],
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
    platforms: {
      windows: { label: "Windows", format: "instalador .msi" },
      macos: { label: "macOS", format: "imagem de disco .dmg" },
      "linux-appimage": { label: "Linux", format: ".AppImage, roda em qualquer distribuição" },
      "linux-deb": { label: "Debian e Ubuntu", format: "pacote .deb" },
    },
    beforeTitle: "Antes de instalar",
    steps: [
      {
        title: "Você vai precisar de um servidor",
        body: (
          <>
            O HavenKeys guarda o seu cofre em um <code>havenkeys-server</code>. Rode o seu seguindo o{" "}
            <Ext href={`${DOCS}deployment.md`}>guia de implantação</Ext> (em inglês), ou peça um
            convite a alguém que já roda um. Configure backups antes de guardar qualquer coisa
            importante.
          </>
        ),
      },
      {
        title: "A extensão é compilada a partir do código",
        body: (
          <>
            A extensão do navegador e o host de native messaging ainda não vêm nesses instaladores.
            O <Ext href={`${GH}#readme`}>README</Ext> (em inglês) tem o passo a passo.
          </>
        ),
      },
      {
        title: "Espere um aviso na primeira execução",
        body: "Os instaladores ainda não são assinados, então o Windows SmartScreen e o macOS Gatekeeper vão mostrar um aviso. As versões para Windows e macOS são mais novas e menos testadas que a de Linux.",
      },
    ],
  },

  privacy: {
    title: "Política de Privacidade",
    updated: "Atualizada em 23 de setembro de 2026.",
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
          publicidade nem nenhum outro script de terceiros.
        </p>

        <h2>O app de desktop e a extensão do navegador</h2>
        <p>
          O aplicativo HavenKeys não tem telemetria, analytics nem relatórios de falha. A sua senha
          mestra e a sua Secret Key nunca são enviadas ao servidor nem à extensão do navegador; para
          fazer login, o app envia ao servidor uma chave derivada delas. A sua Secret Key só sai do
          seu dispositivo do jeito que você escolher: no seu Emergency Kit e quando você a digita em
          outro dispositivo seu. As chaves do cofre são geradas no seu dispositivo e nunca saem dele
          sem estarem cifradas.
        </p>
        <p>
          Todo cofre pertence a uma conta em um <code>havenkeys-server</code> operado por você ou
          por quem convidou você. Esse servidor guarda o seu cofre cifrado, que ele não consegue
          decifrar, além dos metadados de que precisa para servi-lo: o ID do cofre, o e-mail da sua
          conta, os parâmetros e o salt da derivação de chaves, a chave do cofre cifrada, as revisões
          dos itens e a quantidade e o tamanho aproximado dos seus itens. Para limitar tentativas de
          login, ele também conta as tentativas que falharam por conta e por endereço de rede, e
          zera essa contagem depois de um login bem-sucedido. O{" "}
          <Ext href={`${DOCS}server-sync.md`}>projeto de sincronização com o servidor</Ext> (em
          inglês) descreve isso em detalhes. Não operamos nenhum servidor hospedado e não temos
          acesso ao seu.
        </p>

        <h2>O que nunca coletamos</h2>
        <ul>
          <li>Senhas mestras</li>
          <li>Chaves de cifragem do cofre</li>
          <li>Senhas, usuários, segredos TOTP ou notas seguras guardados</li>
          <li>Histórico de navegação ou o conteúdo das páginas que você visita</li>
        </ul>

        <h2>Contato</h2>
        <p>
          Dúvidas sobre esta política podem ser enviadas como uma issue no{" "}
          <Ext href={`${GH}/issues`}>GitHub</Ext>.
        </p>
      </>
    ),
  },

  terms: {
    title: "Termos de Serviço",
    updated: "Atualizados em 23 de setembro de 2026.",
    body: (
      <>
        <h2>Licença</h2>
        <p>
          O HavenKeys é software de código aberto, com licença dupla:{" "}
          <Ext href={`${GH}/blob/main/LICENSE-MIT`}>Licença MIT</Ext> e{" "}
          <Ext href={`${GH}/blob/main/LICENSE-APACHE`}>Licença Apache 2.0</Ext>. Você pode usá-lo,
          modificá-lo e redistribuí-lo nos termos de qualquer uma delas.
        </p>
        <p>
          Este site e o app de desktop incorporam as fontes Hanken Grotesk e JetBrains Mono, e este
          site também incorpora a Source Serif 4. As três são licenciadas à parte sob a SIL Open
          Font License 1.1. Os avisos de copyright e essa licença estão em{" "}
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
          <Ext href={`${DOCS}deployment.md`}>guia de implantação</Ext> (em inglês), principalmente a
          seção sobre backups, antes de guardar qualquer coisa que você não pode perder.
        </p>

        <h2>Sem serviço, sem conta</h2>
        <p>
          Não operamos uma versão hospedada do HavenKeys e não mantemos contas em seu nome. Baixar
          este software não implica assinatura, SLA nem obrigação de suporte.
        </p>
      </>
    ),
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
