"use client";
import { useState, useEffect } from "react";
import { StatusCard, DataRow, StatusPill } from "./status-card";
import type {
  UserWalletStatus,
  UserSocialLoginStatus,
  UserConnectionStatus,
} from "@/lib/types";
import {
  User,
  Users,
  Github,
  Globe,
  CheckCircle,
  AlertTriangle,
} from "lucide-react";
import { Progress } from "@/components/ui/progress";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { Label } from "@/components/ui/label";
import { Avatar, AvatarFallback, AvatarImage } from "@/components/ui/avatar";
import { cn } from "@/lib/utils";
import { useUserState } from "@/context/userState";

export function UserStatusDashboard() {
  const {
    state: userState,
    setSelectedAuthMethod,
    getWalletConnections,
    getSelectedAuthMethod,
    getSocialConnections,
    getConnectedMethods,
  } = useUserState();

  useEffect(() => {
    console.log("UserStatusDashboard userState", userState);
  }, [userState]);

  const selectedAuthMethod = getSelectedAuthMethod();
  console.log("selectedAuthMethod", selectedAuthMethod);

  // Get connected methods
  const connectedMethods = getConnectedMethods().filter(
    (method) => method.isConnected
  );
  console.log("connectedMethods", connectedMethods);
  const walletConnections = getWalletConnections().filter(
    (wallet) => wallet.isConnected
  );
  console.log("walletConnections", walletConnections);
  const socialConnections = getSocialConnections();

  // All available methods (connected and not connected)
  const allMethods = [
    ...Object.values(userState.connections)
      .filter((conn) => conn.isConnected)
      .map((conn) => ({
        id: conn.walletId || `unknown-${Math.random()}`,
        label:
          conn.loginType === "wallet"
            ? `${(conn as UserWalletStatus).wallet} (${
                (conn as UserWalletStatus).chain
              })`
            : `${(conn as UserSocialLoginStatus).provider}`,
        type: conn.loginType,
        connected: conn.isConnected,
        connection: conn,
      })),
  ];

  // Calculate Shamir shares progress
  const totalShares = userState.selectedAuthMethod?.shamirShares?.length || 0;
  const shamirProgress = totalShares > 0 ? (totalShares / 16) * 100 : 0;

  // Get Mina public key from selected auth method
  const getMinaPublicKey = () => {
    return userState.selectedAuthMethod?.minaPublicKey || "Not available";
  };

  const handleAuthMethodChange = (methodId: string) => {
    setSelectedAuthMethod(userState.connections[methodId]);
    const connection = Object.values(userState.connections).find(
      (conn) => conn.walletId === methodId
    );
    if (connection) {
      setSelectedAuthMethod(connection);
    }
  };

  return (
    <StatusCard
      title="User Authentication Status"
      icon={User}
      description="Your connected accounts and security setup."
    >
      <div className="space-y-3">
        <DataRow
          label="Connected Authentication Methods"
          value={`${connectedMethods.length} method${
            connectedMethods.length !== 1 ? "s" : ""
          }`}
          valueClassName="font-semibold"
        />

        {allMethods.length > 0 && selectedAuthMethod && (
          <div className="space-y-2">
            <div className="text-xs font-medium text-muted-foreground">
              Authentication methods:
            </div>
            <RadioGroup
              value={selectedAuthMethod.walletId || ""}
              onValueChange={handleAuthMethodChange}
              className="space-y-1"
            >
              {allMethods.map((method) => (
                <div
                  key={method.id}
                  className={cn(
                    "flex items-center space-x-2",
                    !method.connected && "opacity-40"
                  )}
                >
                  <RadioGroupItem
                    value={method.id}
                    id={`radio-${method.id}`}
                    disabled={!method.connected}
                  />
                  <Label
                    htmlFor={`radio-${method.id}`}
                    className={cn(
                      "text-xs cursor-pointer",
                      method.connected
                        ? "text-foreground"
                        : "text-neutral-600 cursor-not-allowed"
                    )}
                  >
                    {method.label} ({method.type})
                    {!method.connected && " - Not Connected"}
                  </Label>
                </div>
              ))}
            </RadioGroup>
          </div>
        )}

        <DataRow
          label="Mina Address"
          value={getMinaPublicKey()}
          truncate={true}
        />

        <div className="pt-1">
          <div className="flex justify-between items-center mb-1">
            <span className="text-xs font-medium text-muted-foreground">
              Shamir Secret Sharing:
            </span>
            <span className="text-xs text-foreground">
              {totalShares} of 16 Shares Used
            </span>
          </div>
          <Progress
            value={shamirProgress}
            className="w-full h-1.5 bg-muted [&>div]:bg-gradient-to-r [&>div]:from-brand-pink [&>div]:via-brand-purple [&>div]:to-brand-blue"
          />

          {totalShares > 0 && (
            <div className="mt-2">
              <div className="text-xs text-muted-foreground mb-1">
                Share numbers used:
              </div>
              <div className="flex flex-wrap gap-1">
                {userState.selectedAuthMethod?.shamirShares?.map((shareNum) => (
                  <span
                    key={shareNum}
                    className="inline-flex items-center px-1.5 py-0.5 rounded text-xs bg-muted text-muted-foreground border border-border"
                  >
                    #{shareNum}
                  </span>
                ))}
              </div>
            </div>
          )}
        </div>

        {/* Social Logins Section */}
        <div className="space-y-2 pt-2">
          <h4 className="text-sm font-medium text-muted-foreground flex items-center">
            <Users className="h-4 w-4 mr-2" /> Social Logins
          </h4>
          {socialConnections.map((social) => {
            const Icon = social.provider === "github" ? Github : Globe;
            return (
              <div
                key={social.walletId}
                className="p-2 rounded-md bg-muted/30 border border-border backdrop-blur-sm"
              >
                <div className="flex items-center justify-between mb-1.5">
                  <div className="flex items-center space-x-2">
                    <Icon
                      className={`h-5 w-5 ${
                        social.isLoggedIn
                          ? "text-brand-green"
                          : "text-muted-foreground"
                      }`}
                    />
                    <span className="text-sm font-medium text-foreground">
                      {social.provider}
                    </span>
                  </div>
                  <StatusPill
                    status={social.isLoggedIn ? "success" : "info"}
                    text={social.isLoggedIn ? "Connected" : "Not Connected"}
                  />
                </div>
                {social.isLoggedIn && (
                  <div className="space-y-1 text-xs pl-1">
                    <div className="flex items-center space-x-2">
                      <Avatar className="h-5 w-5">
                        <AvatarImage
                          src={social.avatarUrl || "/placeholder.svg"}
                          alt={social.username}
                        />
                        <AvatarFallback>
                          {social.username?.[0].toUpperCase()}
                        </AvatarFallback>
                      </Avatar>
                      <DataRow
                        label="User"
                        value={social.username || "Unknown"}
                        className="border-none py-0.5"
                        valueClassName="text-xs"
                      />
                    </div>
                    <DataRow
                      label="Email"
                      value={social.email || "Not provided"}
                      className="border-none py-0.5"
                      valueClassName="text-xs"
                    />
                    <DataRow
                      label="Session Expires"
                      value={social.sessionExpires || "Unknown"}
                      className="border-none py-0.5"
                      valueClassName="text-xs"
                    />
                  </div>
                )}
              </div>
            );
          })}

          {socialConnections.length === 0 && (
            <div className="text-xs text-muted-foreground text-center py-2">
              No social logins connected
            </div>
          )}
        </div>

        {/* Wallet Connections Section */}
        {walletConnections.length > 0 && (
          <div className="space-y-2 pt-2">
            <h4 className="text-sm font-medium text-muted-foreground flex items-center">
              Wallet Connections
            </h4>
            {walletConnections.map((wallet) => (
              <div
                key={wallet.walletId}
                className="p-2 rounded-md bg-muted/30 border border-border backdrop-blur-sm"
              >
                <div className="flex items-center justify-between mb-1.5">
                  <div className="flex items-center space-x-2">
                    <span className="text-sm font-medium text-foreground">
                      {wallet.wallet}
                    </span>
                    <span className="text-xs text-muted-foreground">
                      ({wallet.chain})
                    </span>
                  </div>
                  <StatusPill status="success" text="Connected" />
                </div>
                <DataRow
                  label="Address"
                  value={wallet.address || "Not available"}
                  className="border-none py-0.5"
                  valueClassName="text-xs"
                  truncate={true}
                />
              </div>
            ))}
          </div>
        )}
      </div>
    </StatusCard>
  );
}
