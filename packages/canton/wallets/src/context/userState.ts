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
import { connectPhantom as phantomConnect, disconnectPhantom } from "@/lib/phantom";
import { connectSolflare as solflareConnect, disconnectSolflare } from "@/lib/solflare";
import { getPartyIdFromSolanaKey } from "@/lib/phantom-mapping";

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

// Connected wallet info type
export interface ConnectedWalletInfo {
  walletType: "loop" | "phantom" | "solflare" | null;
  walletName: string | null;
  partyId: string | null;
  publicKey: string | null;
  solanaPublicKey?: string | null;
}

// Context type
interface UserStateContextType {
  state: UnifiedUserState;
  dispatch: React.Dispatch<UserStateAction>;
  // Loop-specific methods
  connectLoop: (network: "devnet" | "testnet" | "mainnet") => Promise<void>;
  getLoopPartyId: () => string | null;
  // Phantom-specific methods
  connectPhantom: () => Promise<void>;
  getPhantomPartyId: () => string | null;
  // Solflare-specific methods
  connectSolflare: () => Promise<void>;
  getSolflarePartyId: () => string | null;
  // Unified wallet info
  getConnectedWalletInfo: () => ConnectedWalletInfo;
  disconnectWallet: () => void;
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
  connectPhantom: async () => {},
  getPhantomPartyId: () => null,
  connectSolflare: async () => {},
  getSolflarePartyId: () => null,
  getConnectedWalletInfo: () => ({
    walletType: null,
    walletName: null,
    partyId: null,
    publicKey: null,
  }),
  disconnectWallet: () => {},
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
  const [loopPublicKey, setLoopPublicKey] = useState<string | null>(null);
  const [phantomPartyId, setPhantomPartyId] = useState<string | null>(null);
  const [phantomSolanaKey, setPhantomSolanaKey] = useState<string | null>(null);
  const [solflarePartyId, setSolflarePartyId] = useState<string | null>(null);
  const [solflareSolanaKey, setSolflareSolanaKey] = useState<string | null>(null);
  const [connectedWalletType, setConnectedWalletType] = useState<"loop" | "phantom" | "solflare" | null>(null);

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
              setLoopPublicKey(provider.public_key);
              setConnectedWalletType("loop");
              // Clear phantom state
              setPhantomPartyId(null);
              setPhantomSolanaKey(null);

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
              setLoopPublicKey(null);
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

  // Phantom connection
  const connectPhantom = useCallback(async (): Promise<void> => {
    const walletId = "phantom-solana";

    dispatch({
      type: "SET_CONNECTING",
      payload: { walletId, loginType: "wallet" },
    });

    try {
      const solanaPublicKey = await phantomConnect();

      if (!solanaPublicKey) {
        console.log("[userState] Phantom connection rejected or failed");
        dispatch({
          type: "SET_CONNECTION_FAILED",
          payload: { walletId },
        });
        return;
      }

      // Look up the Canton party ID from the mapping
      const cantonPartyId = getPartyIdFromSolanaKey(solanaPublicKey);

      if (!cantonPartyId) {
        console.error("[userState] No Canton party ID found for Solana key:", solanaPublicKey);
        dispatch({
          type: "SET_CONNECTION_FAILED",
          payload: { walletId },
        });
        return;
      }

      console.log("[userState] Phantom connected:", solanaPublicKey, "->", cantonPartyId);
      setPhantomSolanaKey(solanaPublicKey);
      setPhantomPartyId(cantonPartyId);
      setConnectedWalletType("phantom");
      // Clear loop state
      setLoopPartyId(null);
      setLoopPublicKey(null);

      const newConnection: UserWalletStatus = {
        loginType: "wallet",
        chain: "solana",
        wallet: "Phantom",
        walletId,
        isConnected: true,
        isConnectionFailed: false,
        isConnecting: false,
        address: cantonPartyId,
        publicKey: solanaPublicKey,
      };

      dispatch({
        type: "SET_WALLET_CONNECTED",
        payload: { walletId, connection: newConnection },
      });
    } catch (error: any) {
      console.error("[userState] Phantom connection error:", error);
      dispatch({
        type: "SET_CONNECTION_FAILED",
        payload: { walletId },
      });
    }
  }, [dispatch]);

  const getPhantomPartyIdValue = useCallback(() => {
    return phantomPartyId;
  }, [phantomPartyId]);

  // Solflare connection
  const connectSolflare = useCallback(async (): Promise<void> => {
    const walletId = "solflare-solana";

    dispatch({
      type: "SET_CONNECTING",
      payload: { walletId, loginType: "wallet" },
    });

    try {
      const solanaPublicKey = await solflareConnect();

      if (!solanaPublicKey) {
        console.log("[userState] Solflare connection rejected or failed");
        dispatch({
          type: "SET_CONNECTION_FAILED",
          payload: { walletId },
        });
        return;
      }

      // Look up the Canton party ID from the mapping
      const cantonPartyId = getPartyIdFromSolanaKey(solanaPublicKey);

      if (!cantonPartyId) {
        console.error("[userState] No Canton party ID found for Solana key:", solanaPublicKey);
        dispatch({
          type: "SET_CONNECTION_FAILED",
          payload: { walletId },
        });
        return;
      }

      console.log("[userState] Solflare connected:", solanaPublicKey, "->", cantonPartyId);
      setSolflareSolanaKey(solanaPublicKey);
      setSolflarePartyId(cantonPartyId);
      setConnectedWalletType("solflare");
      // Clear other wallet states
      setLoopPartyId(null);
      setLoopPublicKey(null);
      setPhantomPartyId(null);
      setPhantomSolanaKey(null);

      const newConnection: UserWalletStatus = {
        loginType: "wallet",
        chain: "solana",
        wallet: "Solflare",
        walletId,
        isConnected: true,
        isConnectionFailed: false,
        isConnecting: false,
        address: cantonPartyId,
        publicKey: solanaPublicKey,
      };

      dispatch({
        type: "SET_WALLET_CONNECTED",
        payload: { walletId, connection: newConnection },
      });
    } catch (error: any) {
      console.error("[userState] Solflare connection error:", error);
      dispatch({
        type: "SET_CONNECTION_FAILED",
        payload: { walletId },
      });
    }
  }, [dispatch]);

  const getSolflarePartyIdValue = useCallback(() => {
    return solflarePartyId;
  }, [solflarePartyId]);

  // Unified wallet info
  const getConnectedWalletInfo = useCallback((): ConnectedWalletInfo => {
    if (connectedWalletType === "loop" && loopPartyId) {
      return {
        walletType: "loop",
        walletName: "Loop",
        partyId: loopPartyId,
        publicKey: loopPublicKey,
      };
    }
    if (connectedWalletType === "phantom" && phantomPartyId) {
      return {
        walletType: "phantom",
        walletName: "Phantom",
        partyId: phantomPartyId,
        publicKey: phantomPartyId, // For Phantom, publicKey in dashboard context is partyId
        solanaPublicKey: phantomSolanaKey,
      };
    }
    if (connectedWalletType === "solflare" && solflarePartyId) {
      return {
        walletType: "solflare",
        walletName: "Solflare",
        partyId: solflarePartyId,
        publicKey: solflarePartyId, // For Solflare, publicKey in dashboard context is partyId
        solanaPublicKey: solflareSolanaKey,
      };
    }
    return {
      walletType: null,
      walletName: null,
      partyId: null,
      publicKey: null,
    };
  }, [connectedWalletType, loopPartyId, loopPublicKey, phantomPartyId, phantomSolanaKey, solflarePartyId, solflareSolanaKey]);

  // Disconnect current wallet
  const disconnectWallet = useCallback(() => {
    if (connectedWalletType === "phantom") {
      disconnectPhantom();
      setPhantomPartyId(null);
      setPhantomSolanaKey(null);
    }
    if (connectedWalletType === "solflare") {
      disconnectSolflare();
      setSolflarePartyId(null);
      setSolflareSolanaKey(null);
    }
    if (connectedWalletType === "loop") {
      setLoopPartyId(null);
      setLoopPublicKey(null);
    }
    setConnectedWalletType(null);
    dispatch({ type: "RESET_ALL_CONNECTIONS" });
  }, [connectedWalletType, dispatch]);

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
    connectPhantom,
    getPhantomPartyId: getPhantomPartyIdValue,
    connectSolflare,
    getSolflarePartyId: getSolflarePartyIdValue,
    getConnectedWalletInfo,
    disconnectWallet,
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
