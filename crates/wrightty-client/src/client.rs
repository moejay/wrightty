use jsonrpsee::core::client::ClientT;
use jsonrpsee::core::params::ObjectParams;
use jsonrpsee::core::traits::ToRpcParams;
use jsonrpsee::ws_client::{WsClient, WsClientBuilder};
use serde::Serialize;

use wrightty_protocol::methods::*;
use wrightty_protocol::types::*;

pub struct WrighttyClient {
    client: WsClient,
}

/// Wrapper to serialize a struct as named JSON-RPC params (object).
struct NamedParams(serde_json::Value);

impl ToRpcParams for NamedParams {
    fn to_rpc_params(
        self,
    ) -> Result<Option<Box<serde_json::value::RawValue>>, serde_json::Error> {
        let s = serde_json::to_string(&self.0)?;
        let raw = serde_json::value::RawValue::from_string(s)?;
        Ok(Some(raw))
    }
}

fn to_params<T: Serialize>(val: &T) -> Result<NamedParams, Box<dyn std::error::Error>> {
    Ok(NamedParams(serde_json::to_value(val)?))
}

impl WrighttyClient {
    pub async fn connect(url: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let client = WsClientBuilder::default().build(url).await?;
        Ok(Self { client })
    }

    pub async fn get_info(&self) -> Result<ServerInfo, Box<dyn std::error::Error>> {
        let result: GetInfoResult = self
            .client
            .request("Wrightty.getInfo", ObjectParams::new())
            .await?;
        Ok(result.info)
    }

    pub async fn session_create(
        &self,
        params: SessionCreateParams,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let result: SessionCreateResult = self
            .client
            .request("Session.create", to_params(&params)?)
            .await?;
        Ok(result.session_id)
    }

    pub async fn session_destroy(
        &self,
        session_id: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let params = SessionDestroyParams {
            session_id: session_id.to_string(),
            signal: None,
        };
        let _: SessionDestroyResult = self
            .client
            .request("Session.destroy", to_params(&params)?)
            .await?;
        Ok(())
    }

    pub async fn session_list(&self) -> Result<Vec<SessionInfo>, Box<dyn std::error::Error>> {
        let result: SessionListResult = self
            .client
            .request("Session.list", ObjectParams::new())
            .await?;
        Ok(result.sessions)
    }

    pub async fn send_keys(
        &self,
        session_id: &str,
        keys: Vec<KeyInput>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let params = InputSendKeysParams {
            session_id: session_id.to_string(),
            keys,
        };
        let _: serde_json::Value = self
            .client
            .request("Input.sendKeys", to_params(&params)?)
            .await?;
        Ok(())
    }

    pub async fn send_text(
        &self,
        session_id: &str,
        text: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let params = InputSendTextParams {
            session_id: session_id.to_string(),
            text: text.to_string(),
        };
        let _: serde_json::Value = self
            .client
            .request("Input.sendText", to_params(&params)?)
            .await?;
        Ok(())
    }

    pub async fn get_text(
        &self,
        session_id: &str,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let params = ScreenGetTextParams {
            session_id: session_id.to_string(),
            region: None,
            trim_trailing_whitespace: true,
        };
        let result: ScreenGetTextResult = self
            .client
            .request("Screen.getText", to_params(&params)?)
            .await?;
        Ok(result.text)
    }

    pub async fn resize(
        &self,
        session_id: &str,
        cols: u16,
        rows: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let params = TerminalResizeParams {
            session_id: session_id.to_string(),
            cols,
            rows,
        };
        let _: serde_json::Value = self
            .client
            .request("Terminal.resize", to_params(&params)?)
            .await?;
        Ok(())
    }

    pub async fn get_size(
        &self,
        session_id: &str,
    ) -> Result<(u16, u16), Box<dyn std::error::Error>> {
        let params = TerminalGetSizeParams {
            session_id: session_id.to_string(),
        };
        let result: TerminalGetSizeResult = self
            .client
            .request("Terminal.getSize", to_params(&params)?)
            .await?;
        Ok((result.cols, result.rows))
    }
}
