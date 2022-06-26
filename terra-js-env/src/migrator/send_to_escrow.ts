import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {


  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let user = new Address(env['mnemonics'][1]);
  // Uploading the contract code
  let escrow = user.getContract(env.contracts.escrow);
  let escrow_handler = handler.getContract(env.contracts.escrow);


  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft1_minter = handler.getContract(cw721_tokens[cw721_token_names[0]]);
  let nft2_minter = handler.getContract(cw721_tokens[cw721_token_names[1]]);
  let nft1 = user.getContract(cw721_tokens[cw721_token_names[0]]);
  let nft2 = handler.getContract(cw721_tokens[cw721_token_names[1]]);


  /*
    'mario_the_best34369973',
    'mario_the_best357998324',
    'mario_the_best429078238'



*/
  let token_id = "mario_the_best323261604";
  // Then we test the flow
  
  console.log("Send the nft to the escrow contract")
  // First we send the nft to the escrow contract
  let response = await nft1.execute.send_nft({
    contract: escrow.address,
    msg: btoa(JSON.stringify({
      deposit_nft: {
        token_id
      }
    })),
    token_id
  })
  console.log(response);  
} 

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
