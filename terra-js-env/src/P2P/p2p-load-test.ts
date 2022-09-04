import { Address } from '../terra_utils';
import { env } from '../env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let counter = new Address(env['mnemonics'][1]);
  console.log(handler.getAddress());
  let cw20_tokens = env['cw20'];
  let cw20_token_names = Object.keys(cw20_tokens);
  // Uploading the contract code
  //let iliq_codeId: string[] = await handler.uploadContract("../artifacts/iliq_token.wasm");
  //let multisender_codeId: string[] = await handler.uploadContract("../artifacts/multisender.wasm");

  let iliq = handler.getContract(cw20_tokens[cw20_token_names[0]]);
  let p2p = handler.getContract(env.contracts.p2p);
  let p2p_counter = counter.getContract(env.contracts.p2p);

  let response: any;

  // We try to add trades
  response = await p2p.execute.create_trade();
  let trade_id = parseInt(getContractLog(response).trade_id[0]);
  console.log('Created trade', getContractLog(response));

  // We add funds
  // First we approve the contract for the amount
  let amount: string = '500';
  await iliq.execute.increase_allowance({
    spender: p2p.address,
    amount: amount
  });
  // Then we add the funds
  response = await p2p.execute.add_asset({
    trade_id: trade_id,
    asset: {
      cw20_coin: {
        address: cw20_tokens[cw20_token_names[0]],
        amount: amount
      }
    }
  });
  console.log('Added token', getContractLog(response));

  // We confirm our trade !
  response = await p2p.execute.confirm_trade({
    trade_id: trade_id
  });
  console.log('Confirmed Trade', getContractLog(response));

  // Now we create 1000 trades in a single transaction
  let txArray: MsgExecuteContract[] = [];
  for (let i = 0; i < 1000; i++) {
    txArray.push(
      new MsgExecuteContract(
        handler.getAddress(), // sender
        p2p.address, // contract account address
        { create_trade: {} } // handle msg
      )
    );
  }
  response = await p2p.execute.executeSome(txArray);
  console.log(response);

  // We confirm another trade !
  response = await p2p.execute.confirm_trade({
    trade_id: trade_id + 1
  });

  // Now we create 1000 counter_trades in a single transaction
  txArray = [];
  for (let i = 0; i < 1000; i++) {
    txArray.push(
      new MsgExecuteContract(
        handler.getAddress(), // sender
        p2p.address, // contract account address
        {
          suggest_counter_trade: {
            trade_id: trade_id
          }
        } // handle msg
      )
    );
  }
  response = await p2p.execute.executeSome(txArray);

  await p2p.execute.confirm_counter_trade({
    trade_id: trade_id,
    counter_id: 150
  });

  await p2p.execute.confirm_counter_trade({
    trade_id: trade_id,
    counter_id: 151
  }); 

  await p2p.execute.accept_trade({
    trade_id: trade_id,
    counter_id: 151
  });

  // Now the counter does the same !
	await iliq.execute.increase_allowance({
    spender: p2p.address,
    amount: amount
  });
  // Then we add the funds
  response = await p2p.execute.add_asset({
    trade_id: trade_id + 60,
    asset: {
      cw20_coin: {
        address: cw20_tokens[cw20_token_names[0]],
        amount: amount
      }
    }
  });
  await p2p.execute.confirm_trade({
    trade_id: trade_id + 60
  });
  txArray = [];
  for (let i = 0; i < 1000; i++) {
    txArray.push(
      new MsgExecuteContract(
        counter.getAddress(), // sender
        p2p.address, // contract account address
        {
          suggest_counter_trade: {
            trade_id: trade_id + 60
          }
        } // handle msg
      )
    );
  }
  response = await p2p_counter.execute.executeSome(txArray);

  await p2p_counter.execute.confirm_counter_trade({
    trade_id: trade_id + 60,
    counter_id: 150
  });

  await p2p_counter.execute.confirm_counter_trade({
    trade_id: trade_id + 60,
    counter_id: 151
  }); 


}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
