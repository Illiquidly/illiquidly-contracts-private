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


  // First we check if there is a token available on Terra1 for the user

  let response = await nft1.query.tokens({ owner: user.getAddress() });
  console.log(response);
  
  // If not, we just mint them a new token
  let token_id;
  if (response.tokens.length == 0) {
    console.log('Mint new token');
    token_id = user.getAddress() + Math.ceil(Math.random() * 10000);
    await nft1_minter.execute.mint({
      token_id: token_id,
      owner: user.getAddress(),
      token_uri: 'testing'
    });
  } else {
    token_id = response.tokens[0];
  }
  
  // Then we test the flow
  
  console.log("Send the nft to the escrow contract")
  // First we send the nft to the escrow contract
  response = await nft1.execute.send_nft({
    contract: escrow.address,
    msg: btoa(JSON.stringify({
      deposit_nft: {
        token_id
      }
    })),
    token_id
  })
  //console.log(response);
  /*
  console.log("We query the new depositor")
  // Then we try to query if the NFT was actually deposited
  response = await escrow.query.depositor({
    token_id
  });
  if(response.depositor != user.getAddress()){
    console.log("This token was not deposited, don't try to scam the platform");
  }

  // If the NFT was actually deposited, we can transfer the token on Terra 2.0 to our depositor
  // We check the token id exists and belongs to the recipient
  response = await nft2.query.tokens({ owner: handler.getAddress() });
  console.log(response);
  if(!response.tokens.includes(token_id)){
    console.log("No such token_id, creating one for testing");
    await nft2_minter.execute.mint({
      token_id,
      owner: handler.getAddress(),
      token_uri: 'testing'
    });
  }


  response = await nft2.execute.transfer_nft({
    recipient: user.getAddress(),
    token_id,
  })


  console.log(await nft2.query.tokens({
    owner: user.getAddress()
  }))

  // We indicate the token has been migrated
  await escrow_handler.execute.migrated({
    token_id
  })
  */
} 

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
