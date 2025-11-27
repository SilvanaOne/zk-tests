"use client";
import { useEffect } from "react";

export default function PopupDone() {
  useEffect(() => {
    window.opener?.postMessage("oauth-ok", window.location.origin);
    //window.close();
  }, []);

  return null;
}
