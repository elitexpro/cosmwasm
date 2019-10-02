use crate::types::{CosmosMsg, InitParams, SendAmount, SendParams};
use crate::storage::Storage;

use failure::{bail, Error};
use serde::{Deserialize, Serialize};
use serde_json::{from_slice, to_vec};

#[derive(Serialize, Deserialize)]
struct RegenInitMsg {
    verifier: String,
    beneficiary: String,
}

#[derive(Serialize, Deserialize)]
struct RegenState {
    verifier: String,
    beneficiary: String,
    payout: u64,
    funder: String,
}

#[derive(Serialize, Deserialize)]
struct RegenSendMsg {}

pub fn init<T: Storage>(mut store: T, params: InitParams, msg: Vec<u8>) -> Result<Vec<CosmosMsg>, Error> {
    let msg: RegenInitMsg = from_slice(&msg)?;
    store.set_state(to_vec(&RegenState {
        verifier: msg.verifier,
        beneficiary: msg.beneficiary,
        payout: params.sent_funds,
        funder: params.sender
    })?);

    Ok(Vec::new())
}

pub fn send<T:Storage>(mut store: T, params: SendParams, _: Vec<u8>) -> Result<Vec<CosmosMsg>, Error> {
    let data = store.get_state();
    let mut state: RegenState = match data {
        Some(v) => from_slice(&v)?,
        None => { bail!("Not initialized") }
    };
    let funds = state.payout + params.sent_funds;
    state.payout = 0;
    store.set_state(to_vec(&state)?);

    if params.sender == state.verifier {
        Ok(vec![CosmosMsg::SendTx {
            from_address: params.contract_address,
            to_address: state.beneficiary,
            amount: vec![SendAmount {
                denom: "earth".into(),
                amount: funds.to_string(),
            }],
        }])
    } else {
        bail!("Unauthorized")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{Storage, MockStorage};

    #[test]
    fn proper_initialization() {
        let mut store = MockStorage::new();
        let msg = serde_json::to_vec(&RegenInitMsg{
            verifier: String::from("verifies"),
            beneficiary: String::from("benefits"),
        }).unwrap();
        let params = InitParams {
            contract_address: String::from("contract"),
            sender: String::from("creator"),
            sent_funds: 1000,
        };
        let res = init(&mut store, params, msg).unwrap();
        assert_eq!(0, res.len());

        // it worked, let's check the state
        let data = (&mut store).get_state();
        let state: RegenState = match data {
            Some(v) => from_slice(&v).unwrap(),
            _ => panic!("no data stored"),
        };
        assert_eq!(state.payout, 1000);
        assert_eq!(state.verifier, String::from("verifies"));
        assert_eq!(state.beneficiary, String::from("benefits"));
        assert_eq!(state.funder, String::from("creator"));
    }

    #[test]
    fn fails_on_bad_init() {
        let mut store = MockStorage::new();
        let bad_msg = b"{}".to_vec();
        let params = InitParams {
            contract_address: String::from("contract"),
            sender: String::from("creator"),
            sent_funds: 1000,
        };
        let res = init(&mut store, params, bad_msg);
        if let Ok(_) = res {
            assert!(false);
        }
    }
}