# PreapprovalTransferRequest

## Properties

Name | Type | Description | Notes
------------ | ------------- | ------------- | -------------
**jwt** | Option<**String**> | JWT for Ledger API authentication (optional, falls back to JWT_PROVIDER) | [optional]
**sender_party_id** | **String** | Party ID of the sender (external party) | 
**sender_private_key** | **String** | Base58-encoded Ed25519 private key of the sender | 
**receiver_party_id** | **String** | Party ID of the receiver | 
**amount** | **String** | Amount to transfer (in CC) | 
**description** | Option<**String**> | Transfer description/reason | [optional]

[[Back to Model list]](../README.md#documentation-for-models) [[Back to API list]](../README.md#documentation-for-api-endpoints) [[Back to README]](../README.md)


