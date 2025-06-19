"use client";

import React, {
  createContext,
  useReducer,
  useContext,
  ReactNode,
  useCallback,
} from "react";
import { connectWallet, signWalletMessage, getWalletById } from "@/lib/wallet";
import { login } from "@/lib/login";
import type {
  UnifiedUserState,
  UserConnectionStatus,
  UserWalletStatus,
  UserSocialLoginStatus,
  WalletConnectionResult,
  ApiFunctions,
} from "@/lib/types";

// Mock attestation response for addresses
const mockAttestationResponse = {
  addresses: {
    solana_address: "AUJTAeQFrVEoRjKjsKRHaW1aiJG2A5BceSTvGZfpcP1S",
    sui_address:
      "0xa9785af780b16b646041d260c19b2087cac4ffeff636b0347f0b07eee8b0d8f1",
    mina_address: "B62qqngPFeyNniTX8yaTA8S5MxuM2FZrFb2VEsZ3oZ3HudKLBCs4Em3",
    ethereum_address: "0x0ea8643911f36cc73b473735ca2578bb070598b0",
  },
};

const initialUserState: UnifiedUserState = {
  connections: {},
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
  connect: (walletId: string) => Promise<void>;
  getConnectionState: (walletId: string) => WalletConnectionResult;
  resetConnection: (walletId: string) => void;
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
  resetConnection: () => {},
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
    async (walletId: string): Promise<void> => {
      // Get wallet information from the wallet registry
      const walletInfo = getWalletById(walletId);
      if (!walletInfo) {
        console.error(`Wallet with id ${walletId} not found`);
        return;
      }

      // Set connecting state first
      dispatch({
        type: "SET_CONNECTING",
        payload: {
          walletId,
          loginType: walletInfo.type === "social" ? "social" : "wallet",
        },
      });

      try {
        const address = await connectWallet(walletId);
        if (!address && walletInfo.type !== "social") {
          console.log(`Connection rejected for ${walletId}`);
          dispatch({
            type: "SET_CONNECTION_FAILED",
            payload: { walletId },
          });
          return;
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
            // Mock successful social login
            const result = {
              success: true,
              data: ["mock", "social", "shares"],
            };

            if (result.success && result.data) {
              const publicKey = await apiFunctions.decryptShares(
                result.data,
                keyResult.privateKeyId
              );

              // Create social login connection
              const newConnection: UserSocialLoginStatus = {
                loginType: "social",
                provider: walletInfo.provider as "google" | "github",
                walletId,
                isConnected: true,
                isConnecting: false,
                address: publicKey || undefined,
                minaPublicKey: mockAttestationResponse.addresses?.mina_address,
                shamirShares: Array.from(
                  { length: Math.floor(Math.random() * 5) + 1 },
                  () => Math.floor(Math.random() * 16) + 1
                ).sort((a, b) => a - b),
                isLoggedIn: true,
                username: `${walletInfo.provider}User${Math.floor(
                  Math.random() * 1000
                )}`,
                email: `${walletInfo.provider?.toLowerCase()}user@example.com`,
                sessionExpires: new Date(
                  Date.now() + 3600 * 1000
                ).toLocaleString(),
              };

              dispatch({
                type: "SET_SOCIAL_CONNECTED",
                payload: { walletId, connection: newConnection },
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
              return;
            }

            const result = await login(loginRequest);
            if (!result) {
              console.error("Failed to login");
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
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
            return;
          }

          return;
        } else {
          console.log("No API functions provided, setting connection to false");
          dispatch({
            type: "SET_CONNECTION_FAILED",
            payload: { walletId },
          });
        }
      } catch (error) {
        console.error(`Wallet connection error for ${walletId}:`, error);
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

  const setSelectedAuthMethod = useCallback(
    (connection: UserConnectionStatus | null) => {
      dispatch({
        type: "SET_SELECTED_AUTH_METHOD",
        payload: { connection },
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
    resetConnection,
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
