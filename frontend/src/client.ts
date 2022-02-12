import {
  getChainOptions,
  WalletController,
  createLCDClient,
  CreateTxFailed,
  Timeout,
  TxFailed,
  TxResult,
  TxUnspecifiedError,
  UserDenied,
} from '@terra-money/wallet-controller';

import { Fee, MsgSend, MsgExecuteContract } from '@terra-money/terra.js';

import { mergeMap, from } from "rxjs";

import { getController } from 'controller';

const TEST_TO_ADDRESS = 'terra12hnhh5vtyg5juqnzm43970nh4fw42pt27nw9g9';

export function sendSomeLuna(){

  return getController().connectedWallet().pipe(mergeMap(async _connectedWallet => {
    if(!_connectedWallet) return;

    let response = await _connectedWallet.post({
      msgs:[
       new MsgSend(_connectedWallet.terraAddress, TEST_TO_ADDRESS, {
          uusd: 10,
        }),]
    })
    return response;
  }));
}

export function sendNFT(contractAddress: string, id: number){

  return getController().connectedWallet().pipe(mergeMap(async _connectedWallet => {
    if(!_connectedWallet) return;

    let response = await _connectedWallet.post({
      msgs:[
      new MsgExecuteContract(
        _connectedWallet.terraAddress,
        contractAddress,
        {
          transfer:{
            recipient: _connectedWallet.terraAddress,
            id: id
          }
        }
       )
       ]
    })
    return response;
  }));
}

export function getBalances(){
  return getController().connectedWallet().pipe(
    mergeMap(async _connectedWallet => {
      let balance;
      if (_connectedWallet) {
        const lcd = createLCDClient({ network: _connectedWallet.network });
        balance = await lcd.bank.balance(_connectedWallet.terraAddress);
      } else {
        balance = null
      }
      return balance;
    })
  );
}

export function handleTxError(error: Error){
    if (error instanceof UserDenied) {
      return 'User Denied';
    } else if (error instanceof CreateTxFailed) {
      return 'Create Tx Failed: ' + error.message;
    } else if (error instanceof TxFailed) {
      return 'Tx Failed: ' + error.message;
    } else if (error instanceof Timeout) {
      return 'Timeout';
    } else if (error instanceof TxUnspecifiedError) {
      return 'Unspecified Error: ' + error.message;
    } else {
      return 
        'Unknown Error: ' +
        (error instanceof Error ? error.message : String(error));
    }
}