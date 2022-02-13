import {
  getChainOptions,
  WalletController,
} from '@terra-money/wallet-controller';

let instance: WalletController;
let instancePromise: Promise<WalletController>;

export async function initController() {
  instancePromise = getChainOptions()
  .then(chainOptions=>{
    instance = new WalletController({
      ...chainOptions,
    });
    return instance
  })
  return await instancePromise;
}

export function getController(): WalletController {
  return instance;
}

export function getControllerAsync(): Promise<WalletController> {
  return instancePromise;
}
