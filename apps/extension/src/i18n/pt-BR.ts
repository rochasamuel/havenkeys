/*
 * Português do Brasil. Same shape as en.ts, enforced by the Messages type.
 * Terms: senha, cofre, login, código de verificação, chave de acesso
 * (passkey). Product names stay in English.
 */
import type { Messages } from "./en";

export const ptBR: Messages = {
  popup: {
    settings: "Configurações",
    lock: "Bloquear o HavenKeys",
    unreachable: "Não foi possível acessar a extensão.",
    noUsername: "Sem usuário",
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
};
