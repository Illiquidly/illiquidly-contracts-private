
import {settenProject, settenKey } from "./setten-env.json";

export const chains: any = {
  testnet: {
    //URL: 'https://pisco-lcd.terra.dev/',
    URL: `https://lcd.pisco.terra.setten.io/${settenProject}?key=${settenKey}`,
    chainID: 'pisco-1'
  },
  classic: {
    URL: 'https://columbus-lcd.terra.dev',
    chainID: 'columbus-5',
  },
  mainnet: {
    //URL: 'https://phoenix-lcd.terra.dev',
    URL: `https://lcd.phoenix.terra.setten.io/${settenProject}?key=${settenKey}`,
    chainID: 'phoenix-1'
  }
};

export let fcds: any = {
  testnet: 'https://pisco-fcd.terra.dev',
  classic: 'https://columbus-fcd.terra.dev',
  mainnet: 'https://phoenix-fcd.terra.dev'
};

export const registered_nft_contracts: any = "https://assets.terra.money/cw721/contracts.json";