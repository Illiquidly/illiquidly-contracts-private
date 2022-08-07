import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric, MsgExecuteContract } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {

  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress())
  let raffle_contract = handler.getContract(env.contracts.raffle);

  // First we create a raffle

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);
  // First we approve the contract for the NFT

  let token_id = (await nft.query.tokens({owner: handler.getAddress()})).tokens[0]

  let response = await nft.execute.approve({
    spender: raffle_contract.address,
    token_id
  })
  console.log(response)

  response = await raffle_contract.execute.create_raffle({
    asset: {
      cw721_coin:{
        address: nft.address,
        token_id
      }
    },
    raffle_ticket_price: {
      coin:{
        denom: "uluna",
        amount: "1000000"
      }
    },
    raffle_duration: 120,
    raffle_timeout: 30,
    max_participant_number: 4000

  })
  let raffle_id = parseInt(response.logs[0].eventsByType.wasm.raffle_id[0]);

  response = await raffle_contract.query.raffle_info({
     raffle_id,
   })
  console.log(response);


  let buyTicketMsgs: MsgExecuteContract[] = []

  for(var i=0;i<1;i++){
    let msg = {
      buy_ticket: {
         raffle_id,
         sent_assets:{
          coin:{
            denom: "uluna",
            amount: "1000000"
          }
        }
      }
    }

    let executeMsg = new MsgExecuteContract(
      handler.getAddress(),// sender
      raffle_contract.address, // contract address
      { ...msg }, // handle msg,
      "1000000uluna"
    );
    buyTicketMsgs.push(executeMsg);
  }
  for(let i = 0; i < 20; i++) { 
    let fee = await raffle_contract.execute.estimateFee(buyTicketMsgs)
    console.log(fee.gas_limit, fee.amount._coins.uluna);
    let response = await handler.post(buyTicketMsgs)
    console.log(i);
  }



}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
