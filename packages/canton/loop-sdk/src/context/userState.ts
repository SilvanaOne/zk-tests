"use client";

import React, {
  createContext,
  useReducer,
  useContext,
  ReactNode,
  useCallback,
  useState,
} from "react";
import { getWalletById } from "@/lib/wallet";
import type {
  UnifiedUserState,
  UserConnectionStatus,
  UserWalletStatus,
  UserSocialLoginStatus,
  WalletConnectionResult,
  WalletType,
} from "@/lib/types";
import { initLoop, connectLoop as loopConnect, getLoopPartyId as getPartyId } from "@/lib/loop";

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
  // Loop-specific methods
  connectLoop: (network: "devnet" | "testnet" | "mainnet") => Promise<void>;
  getLoopPartyId: () => string | null;
  // Legacy action methods (kept for compatibility)
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
  connectLoop: async () => {},
  getLoopPartyId: () => null,
  getConnectionState: () => ({ state: "idle" }),
  setConnecting: () => {},
  setConnectionFailed: () => {},
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
}> = ({ children }) => {
  const [state, dispatch] = useReducer(userStateReducer, initialUserState);
  const [loopPartyId, setLoopPartyId] = useState<string | null>(null);

  // Loop connection
  const connectLoop = useCallback(
    async (network: "devnet" | "testnet" | "mainnet"): Promise<void> => {
      const walletId = "loop-canton";

      dispatch({
        type: "SET_CONNECTING",
        payload: { walletId, loginType: "wallet" },
      });

      try {
        // Initialize Loop SDK
        initLoop(
          {
            onConnect: (provider) => {
              console.log("[userState] Loop connected:", provider.party_id);
              setLoopPartyId(provider.party_id);

              const newConnection: UserWalletStatus = {
                loginType: "wallet",
                chain: "canton",
                wallet: "Loop",
                walletId,
                isConnected: true,
                isConnectionFailed: false,
                isConnecting: false,
                address: provider.party_id,
                publicKey: provider.public_key,
              };

              dispatch({
                type: "SET_WALLET_CONNECTED",
                payload: { walletId, connection: newConnection },
              });
            },
            onReject: () => {
              console.log("[userState] Loop connection rejected");
              setLoopPartyId(null);
              dispatch({
                type: "SET_CONNECTION_FAILED",
                payload: { walletId },
              });
            },
          },
          network
        );

        // Trigger the connection popup
        loopConnect();
      } catch (error: any) {
        console.error("[userState] Loop connection error:", error);
        dispatch({
          type: "SET_CONNECTION_FAILED",
          payload: { walletId },
        });
      }
    },
    [dispatch]
  );

  const getLoopPartyIdValue = useCallback(() => {
    return loopPartyId || getPartyId();
  }, [loopPartyId]);

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
    connectLoop,
    getLoopPartyId: getLoopPartyIdValue,
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
