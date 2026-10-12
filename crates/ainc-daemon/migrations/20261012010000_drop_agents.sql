-- The agent features (Tickets, Conversations, Automations, Connections) left the product.
-- Todos, the Calendar and Workspaces stay; everything else goes.
DROP TABLE IF EXISTS
  ticket_activity, ticket_links, tool_effects, agent_credentials, ticket_runs,
  dispatch_outbox, ticket_receipts, comments, tickets,
  automation_history, occurrences, automation_receipts, automations,
  worker_health, conversation_sessions, turns, conversations, assistant_settings,
  legacy_imports, agents, principals
CASCADE;
