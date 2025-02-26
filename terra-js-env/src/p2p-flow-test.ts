import { Address } from './terra_utils';
import { env } from './env_helper';

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let counter = new Address(env['mnemonics'][1]);
  console.log(counter.getAddress(), handler.getAddress());
  let cw20_tokens = env['cw20'];
  let cw20_token_names = Object.keys(cw20_tokens);
  let iliq_token_id = 0;
  // Uploading the contract code
  //let iliq_codeId: string[] = await handler.uploadContract("../artifacts/iliq_token.wasm");
  //let multisender_codeId: string[] = await handler.uploadContract("../artifacts/multisender.wasm");

  let iliq = handler.getContract(cw20_tokens[cw20_token_names[iliq_token_id]]);
  let iliq_counter = counter.getContract(cw20_tokens[cw20_token_names[iliq_token_id]]);
  let p2p = handler.getContract(env.contracts.p2p);
  let p2p_counter = counter.getContract(env.contracts.p2p);
  let fee = handler.getContract(env.contracts.fee);
  let fee_counter = counter.getContract(env.contracts.fee);
  let fee_distributor = handler.getContract(env.contracts.fee_distributor);
  
  let response: any;

  response = await p2p.query.contract_info();
  console.log(response);

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

  // No we add an NFT !
  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);
 
  // We mint a new one
  let token_id = handler.getAddress() + Math.ceil(Math.random() * 10000)
  await nft.execute.mint({
      token_id: token_id,
      owner: handler.getAddress(),
      token_uri: 'testing'
    });

  await nft.execute.approve({
    spender: p2p.address,
    token_id: token_id
  });

  // Then we add the funds
  
  response = await p2p.execute.add_asset({
    trade_id: trade_id,
    asset: {
      cw721_coin: {
        address: nft.address,
        token_id
      }
    }
  });  
  console.log('Added NFT', getContractLog(response));
  



  // We confirm our trade !
  response = await p2p.execute.confirm_trade({
    trade_id: trade_id
  });
  console.log('Confirmed Trade', getContractLog(response));

  response = await p2p.query.trade_info({
    trade_id: trade_id
  });
  console.log('Trade Info', response);

  // We create a counter trade
  response = await p2p_counter.execute.suggest_counter_trade({
    trade_id: trade_id
  });
  let counter_id = parseInt(getContractLog(response).counter_id[0]);
  console.log('Created counter trade', getContractLog(response));

  // We add funds
  amount = '1500';
  await iliq_counter.execute.increase_allowance({
    spender: p2p.address,
    amount: amount
  });
  // Then we add the funds
  response = await p2p_counter.execute.add_asset({
    trade_id: trade_id,
    counter_id: counter_id,
    asset: {
      cw20_coin: {
        address: cw20_tokens[cw20_token_names[0]],
        amount: amount
      }
    }
  });
  console.log('Added token', getContractLog(response));

  /*
  // Then we add the terra native funds
  response = await p2p_counter.execute.add_asset({
      trade_id: trade_id,
      counter_id: counter_id,
      asset: {
        coin: {
          denom: "uluna",
          amount: "1000"
        }
      }
    },
    {"uluna": "1000"}
  );
  console.log('Added terra native fund', getContractLog(response));
  */

  // We confirm our counter trade !
  response = await p2p_counter.execute.confirm_counter_trade({
    trade_id: trade_id,
    counter_id: counter_id
  });
  console.log('Confirmed Counter Trade', getContractLog(response));

  // We accept the trade
  response = await p2p.execute.accept_trade({
    trade_id: trade_id,
    counter_id: counter_id
  });
  console.log('Accepted Trade', getContractLog(response));

  // We query the fee to pay
  response = await fee.query.fee({
    trade_id: trade_id
  });
  let fee_to_pay = response.fee;
  console.log(response, response.fee);

  // We query the fee to pay
  response = await fee.query.simulate_fee({
    trade_id: trade_id,
    counter_assets: [
      {
        cw20_coin: {
          address: cw20_tokens[cw20_token_names[0]],
          amount: amount
        },
      },
      {
        cw20_coin: {
          address: cw20_tokens[cw20_token_names[0]],
          amount: amount
        },
      },
      {
        cw20_coin: {
          address: cw20_tokens[cw20_token_names[0]],
          amount: amount
        },
      },
      {
        cw20_coin: {
          address: cw20_tokens[cw20_token_names[0]],
          amount: amount
        },
      }
    ]
  });
  console.log(response, response.fee);
  

  response = await fee.query.fee_rates();
  console.log(response);

  // We withdraw the funds
  response = await fee.execute.pay_fee_and_withdraw({
    trade_id: trade_id
  },
  {"uusd": fee_to_pay});
  console.log(response);

  // We withdraw the funds
  response = await fee_counter.execute.pay_fee_and_withdraw({
    trade_id: trade_id
  },
  {"uusd": fee_to_pay});

  response = await iliq.query.balance({ address: handler.getAddress() });
  console.log('trader', response);

  response = await iliq.query.balance({ address: counter.getAddress() });
  console.log('counter', response);
  //We check the token balances

  response = await fee_distributor.query.addresses();
  console.log(response);
  response = await fee_distributor.query.amount({address:nft.address});
  console.log(response);


}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
