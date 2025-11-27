"use client";

import React, {
  createContext,
  useReducer,
  useContext,
  ReactNode,
  useCallback,
} from "react";
import { connectWallet, signWalletMessage, getWalletById } from "@/lib/wallet";
import { getMessage, login, LoginRequest } from "@/lib/login";
import type {
  UnifiedUserState,
  UserConnectionStatus,
  UserWalletStatus,
  UserSocialLoginStatus,
  WalletConnectionResult,
  ApiFunctions,
  SocialLoginData,
  WalletType,
} from "@/lib/types";
import { Logger } from "@logtail/next";

const log = new Logger({
  source: "UserState",
});

const initialUserState: UnifiedUserState = {
  connections: {} as { [key: string]: UserConnectionStatus },
  selectedAuthMethod: null,
};

type UserStateAction =
  | {
      type: "SET_CONNECTING";
      payload: { walletId: string; loginType: "wallet" | "social" };
    }
  | {
      type: "SET_WALLET_CONNECTED";
      payload: { walletId: string; connection: UserWalletStatus };
    }
  | {
      type: "SET_SOCIAL_CONNECTED";
      payload: { walletId: string; connection: UserSocialLoginStatus };
    }
  | {
      type: "SET_CONNECTION_FAILED";
      payload: { walletId: string };
    }
  | {
      type: "SET_SELECTED_AUTH_METHOD";
      payload: { connection: UserConnectionStatus | null };
    }
  | {
      type: "RESET_CONNECTION";
      payload: { walletId: string };
    }
  | {
      type: "RESET_ALL_CONNECTIONS";
    }
  | {
      type: "RESET_FAILED_CONNECTIONS";
    };

const userStateReducer = (
  state: UnifiedUserState,
  action: UserStateAction
): UnifiedUserState => {
  switch (action.type) {
    case "SET_CONNECTING":
      return {
        ...state,
        connections: {
          ...state.connections,
          [action.payload.walletId]: {
            loginType: action.payload.loginType,
            walletId: action.payload.walletId,
            isConnected: false,
            isConnecting: true,
            isConnectionFailed: false,
            address: undefined,
          } as UserConnectionStatus,
        },
      };

    case "SET_WALLET_CONNECTED":
      return {
        ...state,
        connections: {
          ...state.connections,
          [action.payload.walletId]: action.payload.connection,
        },
        selectedAuthMethod: action.payload.connection,
      };

    case "SET_SOCIAL_CONNECTED":
      return {
        ...state,
        connections: {
          ...state.connections,
          [action.payload.walletId]: action.payload.connection,
        },
        selectedAuthMethod: action.payload.connection,
      };

    case "SET_CONNECTION_FAILED":
      return {
        ...state,
        connections: {
          ...state.connections,
          [action.payload.walletId]: {
            ...state.connections[action.payload.walletId],
            isConnected: false,
            isConnecting: false,
            isConnectionFailed: true,
          },
        },
      };

    case "SET_SELECTED_AUTH_METHOD":
      return {
        ...state,
        selectedAuthMethod: action.payload.connection,
      };

    case "RESET_CONNECTION": {
      const { [action.payload.walletId]: removed, ...remainingConnections } =
        state.connections;
      return {
        ...state,
        connections: remainingConnections,
        selectedAuthMethod:
          state.selectedAuthMethod?.walletId === action.payload.walletId
            ? null
            : state.selectedAuthMethod,
      };
    }

    case "RESET_FAILED_CONNECTIONS":
      return {
        ...state,
        connections: Object.fromEntries(
          Object.entries(state.connections).filter(
            ([_, conn]) => !conn.isConnectionFailed
          )
        ),
      };
    case "RESET_ALL_CONNECTIONS":
      return initialUserState;

    default:
      return state;
  }
};

// Context type
interface UserStateContextType {
  state: UnifiedUserState;
  dispatch: React.Dispatch<UserStateAction>;
  apiFunctions: ApiFunctions;
  // Action methods
  connect: (params: {
    walletId: string;
    socialLoginData?: SocialLoginData;
  }) => Promise<void>;
  getConnectionState: (walletId: string) => WalletConnectionResult;
  setConnecting: (walletId: string, walletType: WalletType) => void;
  setConnectionFailed: (walletId: string) => void;
  resetConnection: (walletId: string) => void;
  resetFailedConnections: () => void;
  setSelectedAuthMethod: (connection: UserConnectionStatus | null) => void;
  getWalletConnections: () => UserWalletStatus[];
  getSocialConnections: () => UserSocialLoginStatus[];
  getConnectedMethods: () => UserConnectionStatus[];
  getSelectedAuthMethod: () => UserConnectionStatus | null;
}

// Create context
const UserStateContext = createContext<UserStateContextType>({
  state: initialUserState,
  dispatch: () => {
    console.error(
      "Error: Dispatch UserStateContext called but no provider found"
    );
    return null;
  },
  apiFunctions: {} as ApiFunctions,
  connect: async () => {},
  getConnectionState: () => ({ state: "idle" }),
  setConnecting: (walletId: string, walletType: WalletType) => {},
  setConnectionFailed: (walletId: string) => {},
  resetConnection: () => {},
  resetFailedConnections: () => {},
  setSelectedAuthMethod: () => {},
  getWalletConnections: () => [],
  getSocialConnections: () => [],
  getConnectedMethods: () => [],
  getSelectedAuthMethod: () => null,
});

// Provider component
export const UserStateProvider: React.FC<{
  children: ReactNode;
  apiFunctions: ApiFunctions;
}> = ({ children, apiFunctions }) => {
  const [state, dispatch] = useReducer(userStateReducer, initialUserState);

  const connect = useCallback(
    async (params: {
      walletId: string;
      socialLoginData?: SocialLoginData;
    }): Promise<void> => {
      const { walletId, socialLoginData } = params;
      // Get wallet information from the wallet registry
      const walletInfo = getWalletById(walletId);
      if (!walletInfo) {
        log.error("Wallet not found T101", {
          walletId,
        });
        return;
      }

      // Set connecting state first
      dispatch({
        type: "SET_CONNECTING",
        payload: {
          walletId,
          loginType: walletInfo.type,
        },
      });

      try {
        let address: string | undefined;
        if (walletInfo.type === "social") {
          console.log("Signing in with social provider", walletInfo.provider);
          console.log("social login data", socialLoginData);

          if (!socialLoginData) {
            log.error("Social login not completed T102", {
              walletId,
              provider: walletInfo.provider,
            });
            dispatch({
              type: "SET_CONNECTION_FAILED",
              payload: { walletId },
            });
            return;
          }
          address = socialLoginData.id;
        } else {
          address = await connectWallet(walletId);
          if (!address) {
            log.error("Connection rejected T103", {
              walletId,
            });
            dispatch({
              type: "SET_CONNECTION_FAILED",
              payload: { walletId },
            });
            return;
          }
        }

        console.log("Connected to wallet", walletId, address);

        // If API functions are provided, use them for actual connection logic
        if (apiFunctions) {
          console.log(`Attempting to connect ${walletId} with API functions`);

          // Get private key ID
          const keyResult = await apiFunctions.getPrivateKeyId();
          if (!keyResult) {
            console.log(`Failed to get private key ID for ${walletId}`);
            dispatch({
              type: "SET_CONNECTION_FAILED",
              payload: { walletId },
            });
            return;
          }

          console.log(
            `Got private key ID for ${walletId}:`,
            keyResult.privateKeyId
          );

          // For social logins, we don't need to sign a wallet message
          if (walletInfo.type === "social") {
            const msgData = await getMessage({
              login_type: "social",
              chain: walletInfo.provider,
              wallet: walletInfo.provider,
              address: address,
              publicKey: keyResult.publicKey,
            });
            if (!msgData || !socialLoginData) {
              console.error("Failed to get message");
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
              return;
            }
            const signature =
              walletInfo.provider === "google"
                ? socialLoginData.idToken
                : socialLoginData.accessToken;
            if (!signature) {
              console.error("No access token found");
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
              log.error("No access token found T104", {
                walletId,
              });
              return;
            }
            const loginRequest: LoginRequest = {
              ...msgData.request,
              signature: signature,
              public_key: keyResult.publicKey,
            };
            const result = await login(loginRequest);

            if (result.success && result.data) {
              const publicKey = await apiFunctions.decryptShares(
                result.data,
                keyResult.privateKeyId
              );
              if (!publicKey) {
                console.error("Failed to decrypt shares");
                dispatch({
                  type: "SET_CONNECTION_FAILED",
                  payload: { walletId },
                });
                return;
              }

              // Create social login connection
              const newConnection: UserSocialLoginStatus = {
                loginType: "social",
                provider: walletInfo.provider,
                walletId,
                isConnected: true,
                isConnectionFailed: false,
                isConnecting: false,
                address: address,
                minaPublicKey: publicKey,
                shamirShares: loginRequest.share_indexes,
                isLoggedIn: true,
                username: socialLoginData.name ?? undefined,
                email: socialLoginData.email ?? undefined,
                sessionExpires: new Date(
                  Date.now() + 3600 * 1000
                ).toLocaleString(),
              };

              dispatch({
                type: "SET_SOCIAL_CONNECTED",
                payload: { walletId, connection: newConnection },
              });
              log.info("Social login connected T103", {
                walletId,
                address,
              });
              return;
            }
          } else {
            // Handle wallet connections
            const loginRequest = await signWalletMessage({
              walletId,
              address: address!,
              publicKey: keyResult.publicKey,
            });
            console.log("loginRequest", loginRequest);

            if (!loginRequest) {
              console.error("Failed to sign message");
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
              log.error("Failed to login T105", {
                walletId,
                address,
              });
              return;
            }

            const result = await login(loginRequest);
            if (!result) {
              console.error("Failed to login");
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
              log.error("Failed to login T106", {
                walletId,
                address,
              });
              return;
            }

            if (
              result.success === false ||
              result.data === null ||
              result.data === undefined
            ) {
              console.error("Login error", result.error);
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
              log.error("Login error T107", {
                walletId,
                address,
              });
              return;
            }

            const publicKey = await apiFunctions.decryptShares(
              result.data,
              keyResult.privateKeyId
            );
            if (!publicKey) {
              console.error("Failed to decrypt shares");
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
              log.error("Failed to decrypt shares T108", {
                walletId,
                address,
              });
              return;
            }

            // Create wallet connection using wallet info
            console.log("Creating wallet connection", walletId, address);
            const newConnection: UserWalletStatus = {
              loginType: "wallet",
              chain: walletInfo.chain as "ethereum" | "solana" | "sui",
              wallet: walletInfo.name,
              walletId,
              isConnected: true,
              isConnectionFailed: false,
              isConnecting: false,
              address,
              minaPublicKey: publicKey,
              shamirShares: loginRequest.share_indexes,
            };

            console.log("newConnection", newConnection);
            dispatch({
              type: "SET_WALLET_CONNECTED",
              payload: { walletId, connection: newConnection },
            });
            log.info("Wallet connection created T109", {
              walletId,
              address,
            });
            return;
          }

          return;
        } else {
          console.log("No API functions provided, setting connection to false");
          dispatch({
            type: "SET_CONNECTION_FAILED",
            payload: { walletId },
          });
          log.error("No API functions provided T110", {
            walletId,
          });
        }
      } catch (error: any) {
        console.error(`Wallet connection error for ${walletId}:`, error);
        log.error("Wallet connection error T111", {
          walletId,
          error: error?.message,
        });
        dispatch({
          type: "SET_CONNECTION_FAILED",
          payload: { walletId },
        });
      }
    },
    [dispatch, apiFunctions]
  );

  const getConnectionState = useCallback(
    (walletId: string): WalletConnectionResult => {
      const connection = state.connections[walletId];
      if (!connection) {
        return { state: "idle" };
      }

      return {
        state: connection.isConnecting
          ? "connecting"
          : connection.isConnected
          ? "connected"
          : connection.isConnectionFailed
          ? "error"
          : "idle",
        address: connection.address,
        shamirShares: connection.shamirShares?.map(String),
      };
    },
    [state.connections]
  );

  const resetConnection = useCallback(
    (walletId: string) => {
      dispatch({
        type: "RESET_CONNECTION",
        payload: { walletId },
      });
    },
    [dispatch]
  );

  const resetFailedConnections = useCallback(() => {
    dispatch({
      type: "RESET_FAILED_CONNECTIONS",
    });
  }, [dispatch]);

  const setSelectedAuthMethod = useCallback(
    (connection: UserConnectionStatus | null) => {
      dispatch({
        type: "SET_SELECTED_AUTH_METHOD",
        payload: { connection },
      });
    },
    [dispatch]
  );

  const setConnecting = useCallback(
    (walletId: string, walletType: WalletType) => {
      dispatch({
        type: "SET_CONNECTING",
        payload: { walletId, loginType: walletType },
      });
    },
    [dispatch]
  );

  const setConnectionFailed = useCallback(
    (walletId: string) => {
      dispatch({
        type: "SET_CONNECTION_FAILED",
        payload: { walletId },
      });
    },
    [dispatch]
  );

  // Helper functions to get specific types of connections
  const getWalletConnections = useCallback(() => {
    return Object.values(state.connections).filter(
      (conn): conn is UserWalletStatus =>
        conn.loginType === "wallet" && conn.isConnected
    );
  }, [state.connections]);

  const getSocialConnections = useCallback(() => {
    return Object.values(state.connections).filter(
      (conn): conn is UserSocialLoginStatus =>
        conn.loginType === "social" && conn.isConnected
    );
  }, [state.connections]);

  const getConnectedMethods = useCallback(() => {
    return Object.values(state.connections).filter((conn) => conn.isConnected);
  }, [state.connections]);

  const getSelectedAuthMethod = useCallback(() => {
    return state.selectedAuthMethod;
  }, [state.selectedAuthMethod]);

  const contextValue: UserStateContextType = {
    state,
    dispatch,
    apiFunctions,
    connect,
    getConnectionState,
    setConnecting,
    setConnectionFailed,
    resetConnection,
    resetFailedConnections,
    setSelectedAuthMethod,
    getWalletConnections,
    getSocialConnections,
    getConnectedMethods,
    getSelectedAuthMethod,
  };

  return React.createElement(
    UserStateContext.Provider,
    { value: contextValue },
    children
  );
};

// Hook to access context
export const useUserState = () => useContext(UserStateContext);
