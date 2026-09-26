/*
 * Português do Brasil. Same shape as en.ts, enforced by the Messages type.
 * Terms: senha, cofre, login, código de verificação, chave de acesso
 * (passkey). Product names stay in English.
 */
import type { Messages } from "./en";

export const ptBR: Messages = {
  common: {
    noUsername: "Sem usuário",
    passkey: "Chave de acesso",
  },

  popup: {
    settings: "Configurações",
    lock: "Bloquear o HavenKeys",
    unreachable: "Não foi possível acessar a extensão.",
    fill: "Preencher",
    fillTitle: "Preencher a página com este login",
    code: "Código",
    codeTitle: "Mostrar o código de verificação",
    fillCodeTitle: "Preencher a página com este código",
    pill: {
      offline: "Offline",
      locked: "Bloqueado",
      off: "Desativado",
      unlocked: "Desbloqueado",
    },
    hostUnavailable: {
      title: "Não conectado",
      body: "Instale ou atualize o app HavenKeys neste computador e abra-o uma vez. Ele conecta este navegador para você.",
    },
    desktopUnavailable: {
      title: "O HavenKeys não está aberto",
      body: "Abra o app HavenKeys neste computador.",
    },
    noVault: {
      title: "Nenhum cofre ainda",
      body: "Crie seu cofre no app HavenKeys.",
    },
    locked: {
      title: "O HavenKeys está bloqueado",
      body: "Desbloqueie-o no app HavenKeys para usar seus logins.",
    },
    disabled: {
      title: "A integração com o navegador está desativada",
      body: "Ative-a em HavenKeys → Configurações → Extensão do navegador.",
    },
    error: {
      title: "Algo deu errado",
    },
    noPage: {
      title: "Nenhum login salvo aqui",
      body: "Esta página não pode usar logins salvos.",
    },
    noMatches: {
      title: "Nenhum login salvo para este site",
      body: "Os logins salvos no HavenKeys para este site aparecem aqui.",
    },
    offer: {
      title: "As sugestões nos campos de login estão desativadas",
      body: "Ative-as para escolher logins logo abaixo do campo e para salvar e usar chaves de acesso com o HavenKeys.",
      turnOn: "Ativar",
      turnOnTitle: "Mostrar seus logins abaixo dos campos de login nos sites",
      doneTitle: "Sugestões ativadas",
      doneBody: "Recarregue as abas abertas para usá-las nelas.",
    },
  },

  options: {
    pageTitle: "Configurações do HavenKeys",
    subtitle: "Configurações da extensão do navegador",
    suggestionsTitle: "Sugestões e chaves de acesso",
    toggleLabel: "Mostrar meus logins abaixo dos campos de login e salvar chaves de acesso no HavenKeys",
    notes: [
      "Com isto ativado, ao clicar em um campo de usuário, senha ou código de verificação em um site, seus logins do HavenKeys para esse site aparecem logo abaixo do campo. Nada é preenchido até você escolher um login, e cada login só é oferecido nos sites para os quais foi salvo.",
      "A mesma opção permite que o HavenKeys responda quando um site cria ou pede uma chave de acesso, no lugar do navegador ou do sistema operacional. Sem ela, os sites mostram a janela de chave de acesso do próprio navegador.",
      "Para isso, a extensão precisa de permissão para rodar nos sites que você visita. Ela lê os campos de login quando você interage com eles e nunca envia o conteúdo das páginas a lugar nenhum; o app para computador decide quais logins cada site pode usar.",
    ],
    withoutTitle: "Sem isso",
    withoutBefore: "Clique no botão do HavenKeys na barra de ferramentas e escolha ",
    withoutAfter:
      ". A extensão passa a ter acesso só à aba em que você clicou, e só até você sair da página. Para oferecer salvar novos logins, as sugestões precisam estar ativadas.",
    statusOff: "Desativado. O HavenKeys funciona só pelo botão da barra de ferramentas.",
    statusHttps: "Ativado para sites seguros (https).",
    statusOn: "Ativado. Recarregue as abas abertas para ver as sugestões nelas.",
    turnOn: "Ativar",
    turnOff: "Desativar",
  },

  menu: {
    pageTitle: "Sugestões do HavenKeys",
    lockedTitle: "O HavenKeys está bloqueado",
    lockedBody: "Desbloqueie o app HavenKeys para preencher.",
    unavailable: "Indisponível",
    couldNotFill: "Não foi possível preencher",
    fillCode: "Preencher código de verificação",
    generateTitle: "Gerar senha forte",
    generateBody: "Preenche os campos de nova senha",
    passkeyRow: (account: string) => `Chave de acesso · ${account}`,
    passkeyAccountFallback: "conta",
    usePasskeyTitle: (site: string) => `Você tem uma chave de acesso para ${site}`,
    usePasskeyBody: "Use a opção do site para entrar com chave de acesso",
    addPasskeyTitle: (name: string) => `${name} aceita chaves de acesso`,
    addPasskeyBody: "Como adicionar uma",
    noCodesTitle: "Nenhum código de verificação aqui",
    noCodesBody: "Nenhum login deste site tem código de verificação.",
    noLoginsTitle: "Nenhum login para este site",
    noLoginsBody: "Salve um no app HavenKeys.",
  },

  save: {
    pageTitle: "Salvar no HavenKeys",
    loading: "Salvar este login?",
    addQuestion: "Salvar este login no HavenKeys?",
    updateQuestion: "Atualizar a senha salva?",
    save: "Salvar",
    update: "Atualizar",
    notNow: "Agora não",
  },

  passkey: {
    pageTitle: "Chaves de acesso do HavenKeys",
    loading: "Carregando…",
    useAnotherDevice: "Usar outro dispositivo",
    cancel: "Cancelar",
    save: "Salvar",
    close: "Fechar",
    saveTo: "Salvar em",
    newLogin: "Novo login",
    newLoginDetail: "Criar um login para este site",
    lockedTitle: "O HavenKeys está bloqueado",
    lockedBody: "Desbloqueie o app HavenKeys — este cartão será atualizado.",
    chooserTitle: "Entrar com uma chave de acesso",
    chooseAccount: "Escolha uma conta",
    addTitle: "Adicionar uma chave de acesso?",
    saveTitle: "Salvar uma chave de acesso no HavenKeys?",
    account: (name: string) => `Conta: ${name}`,
    noAccountName: "Sem nome de conta",
    existsTitle: "Esta conta já tem uma chave de acesso no HavenKeys",
    savedTitle: "Chave de acesso salva no HavenKeys",
    savedBody: "Gerencie-a no app HavenKeys",
  },

  content: {
    iconLabel: "HavenKeys: mostrar logins",
  },

  errors: {
    unreachable: "Não foi possível acessar o HavenKeys.",
    generic: "Algo deu errado.",
    invalidRequest: "Solicitação inválida.",
    menuExpired: "Este menu expirou.",
    promptExpired: "Este aviso expirou.",
    unknownItem: "Item desconhecido.",
    unknownPasskey: "Chave de acesso desconhecida.",
    unknownLogin: "Login desconhecido.",
    unknownRequest: "Solicitação desconhecida.",
    pleaseWait: "Aguarde…",
    pageNotSupported: "Esta página não pode usar logins salvos.",
    noLoginForm: "Nenhum formulário de login encontrado nesta página.",
    hostUnavailable: "O host de mensagens nativas do HavenKeys não está instalado ou não conseguiu iniciar.",
    timeout: "O HavenKeys não respondeu.",
    unexpectedResponse: "Resposta inesperada.",
    bridge: {
      locked: "O HavenKeys está bloqueado.",
      busy: "O HavenKeys está ocupado. Tente novamente em instantes.",
      no_vault: "Nenhum cofre foi criado ainda.",
      not_found: "Item não encontrado.",
      denied: "Este item não está salvo para este site.",
      invalid_input: "Solicitação inválida.",
      decryption: "Falha ao descriptografar o item do cofre.",
      corrupted: "O item do cofre está danificado.",
      malformed: "Mensagem malformada.",
      too_large: "Mensagem grande demais.",
      unsupported_version: "Versão de protocolo não suportada.",
      rate_limited: "Muitas solicitações. Tente novamente em breve.",
      integration_disabled: "A integração com o navegador está desativada nas configurações do HavenKeys.",
      desktop_unavailable: "O app HavenKeys não está aberto.",
      offline: "O HavenKeys está offline. O cofre fica somente leitura até reconectar.",
      internal: "Erro interno.",
    },
  },
};
