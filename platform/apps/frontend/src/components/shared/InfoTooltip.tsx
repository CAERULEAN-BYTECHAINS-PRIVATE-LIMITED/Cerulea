"use client";
import * as React from "react";
import Tooltip from "@mui/material/Tooltip";
import IconButton from "@mui/material/IconButton";
import InfoOutlined from "@mui/icons-material/InfoOutlined";

type Props = { title?: string; children?: React.ReactNode; size?: "small"|"medium" };
export default function InfoTooltip({ title, children, size="small" }: Props) {
  const content = (
    <div style={{ maxWidth: 460, lineHeight: 1.5 }}>
      {title && <div style={{ fontWeight: 600, marginBottom: 6 }}>{title}</div>}
      <div>{children}</div>
    </div>
  );
  return (
    <Tooltip title={content} arrow placement="bottom-start">
      <IconButton size={size} color="inherit" aria-label="Info">
        <InfoOutlined fontSize="inherit" />
      </IconButton>
    </Tooltip>
  );
}
